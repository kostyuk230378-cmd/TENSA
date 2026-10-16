/**
 * SSE-клиент Tensa PWA (`GET /api/v1/events`).
 *
 * Ловушка, из-за которой здесь нет `EventSource`: браузерный `EventSource`
 * не умеет передавать заголовки, а сервер требует доступ по схеме Bearer на всех
 * маршрутах `/api/v1`. Поэтому поток читается через `fetch` + `ReadableStream`,
 * а кадры разбираются вручную (см. {@link createSseParser}).
 *
 * Модуль не знает, откуда берутся заголовки: их собирает вызывающий слой
 * (см. `lib/api/auth.js` и `lib/stores/api.js`) и передаёт через `headers`.
 *
 * Формат кадров — стандартный SSE: строка `event: <имя>`, одна или несколько
 * строк `data: <полезная нагрузка>`, кадр завершается пустой строкой.
 * tensa-server шлёт `welcome`, `status:health` и `approval:updated`; строки,
 * начинающиеся с `:`, — keep-alive и в события не попадают.
 */
import { normalizeBaseUrl } from './tensa';

/** Состояние соединения с потоком событий. */
export type EventStreamState =
  | 'connecting'
  | 'open'
  | 'reconnecting'
  | 'unauthorized'
  | 'error'
  | 'closed';

/** Разобранный кадр SSE. */
export interface SseEvent {
  /** Имя события (`event:`), по умолчанию `message`. */
  event: string;
  /** Полезная нагрузка (`data:`); несколько строк склеены через `\n`. */
  data: string;
  /** Идентификатор (`id:`), если сервер его прислал. */
  id?: string;
}

/** Настройки потока событий. */
export interface EventStreamOptions {
  /**
   * Заголовки запроса. Читаются перед каждой попыткой подключения, поэтому
   * смена настроек доступа применяется без пересоздания клиента.
   */
  headers: () => Record<string, string>;
  /** Базовый путь API: строка или геттер (по умолчанию `/api/v1`). */
  baseUrl?: string | (() => string);
  /** Обработчик разобранных событий. */
  onEvent: (event: SseEvent) => void;
  /** Обработчик смены состояния соединения. */
  onState?: (state: EventStreamState, detail?: string) => void;
  /** Подмена fetch (тесты). */
  fetchImpl?: typeof fetch;
  /** Базовая пауза перед повтором после обрыва, мс. */
  retryDelayMs?: number;
  /** Верхняя граница паузы перед повтором, мс. */
  maxRetryDelayMs?: number;
  /** Пауза перед повтором при 401 (доступ не принят сервером), мс. */
  unauthorizedRetryMs?: number;
}

/** Управление потоком: `stop()` закрывает соединение и останавливает повторы. */
export interface EventStreamHandle {
  stop(): void;
}

/** Инкрементальный разборщик кадров SSE: буфер копится до пустой строки. */
export function createSseParser(): { push(chunk: string): SseEvent[] } {
  let buffer = '';

  return {
    push(chunk: string): SseEvent[] {
      buffer += chunk;
      const events: SseEvent[] = [];

      // Кадр завершается пустой строкой; переводы строк могут быть \n и \r\n.
      let separator = findFrameEnd(buffer);
      while (separator) {
        const frame = buffer.slice(0, separator.index);
        buffer = buffer.slice(separator.index + separator.length);
        const event = parseFrame(frame);
        if (event) {
          events.push(event);
        }
        separator = findFrameEnd(buffer);
      }

      return events;
    },
  };
}

/** Найти конец кадра (пустую строку) и вернуть позицию и длину разделителя. */
function findFrameEnd(buffer: string): { index: number; length: number } | null {
  const match = /\r?\n\r?\n/.exec(buffer);
  if (!match || match.index === undefined) {
    return null;
  }
  return { index: match.index, length: match[0].length };
}

/** Разобрать один кадр; `null` — если полезной нагрузки нет (например, keep-alive). */
function parseFrame(frame: string): SseEvent | null {
  let event = 'message';
  let id: string | undefined;
  const dataLines: string[] = [];

  for (const rawLine of frame.split(/\r?\n/)) {
    if (!rawLine || rawLine.startsWith(':')) {
      // Пустая строка и комментарии (в том числе keep-alive) — не событие.
      continue;
    }
    const colon = rawLine.indexOf(':');
    const field = colon === -1 ? rawLine : rawLine.slice(0, colon);
    let value = colon === -1 ? '' : rawLine.slice(colon + 1);
    // По спецификации SSE один ведущий пробел после двоеточия отбрасывается.
    if (value.startsWith(' ')) {
      value = value.slice(1);
    }
    if (field === 'event') {
      event = value;
    } else if (field === 'data') {
      dataLines.push(value);
    } else if (field === 'id') {
      id = value;
    }
  }

  if (dataLines.length === 0 && event === 'message') {
    return null;
  }
  return { event, data: dataLines.join('\n'), id };
}

/**
 * Открыть поток событий с автоматическим переподключением.
 *
 * Отказ доступа (401) не завершает поток: он переходит в состояние
 * `unauthorized` и повторяет попытку реже — интерфейс показывает «токен неверен»
 * и сам восстановится, когда пользователь введёт правильный токен.
 */
export function createEventStream(options: EventStreamOptions): EventStreamHandle {
  const {
    onEvent,
    onState,
    fetchImpl,
    retryDelayMs = 2000,
    maxRetryDelayMs = 15000,
    unauthorizedRetryMs = 10000,
  } = options;
  const doFetch: typeof fetch = fetchImpl ?? ((input, init) => fetch(input, init));
  const readBaseUrl = (): string =>
    normalizeBaseUrl(typeof options.baseUrl === 'function' ? options.baseUrl() : options.baseUrl);

  let stopped = false;
  let controller: AbortController | null = null;

  const setState = (state: EventStreamState, detail?: string): void => {
    onState?.(state, detail);
  };

  async function loop(): Promise<void> {
    let attempt = 0;

    while (!stopped) {
      controller = new AbortController();
      const signal = controller.signal;
      setState(attempt === 0 ? 'connecting' : 'reconnecting', undefined);

      let unauthorized = false;

      try {
        const url = `${readBaseUrl()}/events`;
        const response = await doFetch(url, {
          method: 'GET',
          headers: options.headers(),
          signal,
          cache: 'no-store',
        });

        if (response.status === 401) {
          unauthorized = true;
          setState('unauthorized', await errorText(response));
        } else if (!response.ok) {
          setState('error', `${response.status} ${response.statusText}`);
        } else if (!response.body) {
          setState('error', 'поток событий недоступен: тело ответа пустое');
        } else {
          attempt = 0;
          setState('open', undefined);
          const parser = createSseParser();
          const reader = response.body.getReader();
          const decoder = new TextDecoder();
          try {
            for (;;) {
              const { value, done } = await reader.read();
              if (done) {
                break;
              }
              for (const event of parser.push(decoder.decode(value, { stream: true }))) {
                onEvent(event);
              }
            }
          } finally {
            reader.releaseLock();
          }
          // Сервер закрыл поток (перезапуск, штатное завершение) — переподключаемся.
          setState('reconnecting', 'поток закрыт сервером');
        }
      } catch (error) {
        if (stopped || (error instanceof Error && error.name === 'AbortError')) {
          break;
        }
        setState('error', error instanceof Error ? error.message : String(error));
      }

      if (stopped) {
        break;
      }

      attempt += 1;
      const delay = unauthorized
        ? unauthorizedRetryMs
        : Math.min(retryDelayMs * 2 ** (attempt - 1), maxRetryDelayMs);
      await sleep(delay, signal);
    }

    setState('closed', undefined);
  }

  // Поток открывается сразу: `loop` не бросает наружу, причины видны в onState.
  void loop();

  return {
    stop(): void {
      stopped = true;
      controller?.abort();
    },
  };
}

/** Текст ошибки из ответа: тело `{ error }`, иначе статус. */
async function errorText(response: Response): Promise<string> {
  try {
    const text = await response.text();
    if (!text) {
      return `${response.status} ${response.statusText}`;
    }
    try {
      const parsed = JSON.parse(text) as { error?: string };
      return parsed?.error ?? text;
    } catch {
      return text;
    }
  } catch {
    return `${response.status} ${response.statusText}`;
  }
}

/** Пауза с возможностью прервать её через AbortSignal. */
function sleep(ms: number, signal: AbortSignal): Promise<void> {
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, ms);
    signal.addEventListener(
      'abort',
      () => {
        clearTimeout(timer);
        resolve();
      },
      { once: true },
    );
  });
}
