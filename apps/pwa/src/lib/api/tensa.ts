/**
 * HTTP-клиент Tensa PWA (Контракт 12, `teh_doc/contracts/pwa/openapi.yaml`).
 *
 * Особенности, заданные сервером (`crates/tensa-server`):
 * - базовый путь `/api/v1` на том же origin (PWA раздаёт тот же сервер);
 * - авторизация — `Authorization: Bearer <hex-токен>` из файла `<root>/pwa_token`,
 *   значение пользователь вводит в PWA (хранится в localStorage, см. stores/token.js);
 * - ошибки приходят телом `{ "error": "..." }` с осмысленным HTTP-статусом
 *   (401 — неверный токен, 404, 409 — конфликт перехода статуса, 501 — маршрут
 *   Контракта 12 ещё не реализован в MVP).
 *
 * Клиент не знает про Svelte: адрес и токен передаются функциями-геттерами,
 * поэтому смена настроек применяется к следующему же запросу.
 */

/** Базовый путь API по умолчанию: same-origin, как в контракте. */
export const DEFAULT_BASE_URL = '/api/v1';

/** Тело ошибки, которое отдаёт tensa-server (`error::ErrorBody`). */
export interface ApiErrorBody {
  error?: string;
}

/** Ошибка запроса: HTTP-статус + текст от сервера. */
export class ApiError extends Error {
  /** HTTP-статус ответа (0 — сеть недоступна, запрос не дошёл). */
  readonly status: number;
  /** Разобранное тело ответа (если это был JSON). */
  readonly body: unknown;

  constructor(status: number, message: string, body: unknown = null) {
    super(message);
    this.name = 'ApiError';
    this.status = status;
    this.body = body;
  }

  /** Запрос отклонён из-за неверного или отсутствующего токена. */
  get isUnauthorized(): boolean {
    return this.status === 401;
  }

  /** Маршрут есть в контракте, но в MVP-бэкенде не реализован. */
  get isNotImplemented(): boolean {
    return this.status === 501;
  }

  /** Сеть недоступна (сервер выключен, обрыв Tailscale). */
  get isNetworkError(): boolean {
    return this.status === 0;
  }
}

/** Состояние одного компонента ядра. */
export interface ComponentHealth {
  healthy: boolean;
  detail?: string | null;
}

/** Сводный статус ядра (`GET /health`). */
export interface HealthStatus {
  healthy: boolean;
  version: string;
  uptime: string;
  degraded: boolean;
  degraded_reason?: string | null;
  components?: Record<string, ComponentHealth>;
}

/** Заметка (`GET /notes`). */
export interface Note {
  id: string;
  title: string;
  body: string;
  tags: string[];
  created_at: number;
  updated_at: number;
  created_on_node?: string;
  updated_on_node?: string;
  deleted?: boolean;
}

/** Тело создания/обновления заметки (`POST /notes`, `PUT /notes/{id}`). */
export interface NewNote {
  title: string;
  body: string;
  tags?: string[];
}

/** Действие в очереди одобрения (`GET /approvals`). */
export interface Action {
  id: string;
  kind: string;
  payload: unknown;
  status: string;
  created_at: number;
  created_by?: string;
  created_on_node?: string;
  expires_at: number;
  approved_at?: number | null;
  approved_by?: string | null;
  result?: unknown;
}

/** Узел сети (`GET /nodes`; в MVP-бэкенде маршрут отвечает 501). */
export interface NodeInfo {
  node_id: string;
  name: string;
  device_type?: string;
  platform?: string;
  role?: string;
  tailscale_ip?: string;
  grpc_port?: number;
  last_seen?: number;
  online?: boolean;
  cpu_cores?: number;
  cpu_usage_percent?: number;
  ram_total_mb?: number;
  ram_available_mb?: number;
  has_gpu?: boolean;
  has_npu?: boolean;
  battery_percent?: number | null;
  thermal_status?: string;
}

/** Ответ ядра на запрос к модели (`POST /query`, Контракт 12). */
export interface QueryResponse {
  text: string;
  action?: string | null;
  action_payload?: unknown;
  tool_calls: unknown[];
  reasoning?: string | null;
  degraded: boolean;
  model_used: string;
  node_used: string;
  tokens_used?: number | null;
}

/** Параметры клиента. Значения-функции читаются перед каждым запросом. */
export interface TensaClientOptions {
  /** Базовый путь API: строка или геттер (по умолчанию {@link DEFAULT_BASE_URL}). */
  baseUrl?: string | (() => string);
  /** Bearer-токен: строка, геттер или `null` (запрос уйдёт без заголовка). */
  token?: string | (() => string | null) | null;
  /** Подмена fetch (тесты). */
  fetchImpl?: typeof fetch;
  /** Вызывается при каждом ответе 401 — UI переходит в состояние «токен неверен». */
  onUnauthorized?: () => void;
}

/** Нормализовать базовый путь: убрать хвостовые слэши, пустое значение → дефолт. */
export function normalizeBaseUrl(raw: string | null | undefined): string {
  const value = (raw ?? '').trim();
  if (!value) {
    return DEFAULT_BASE_URL;
  }
  // Разрешаем вводить как «/api/v1», так и «http://100.64.0.1:8765/api/v1».
  return value.replace(/\/+$/, '');
}

/** Собрать query-строку, отбросив пустые значения. */
function buildQuery(params: Record<string, string | number | boolean | undefined | null>): string {
  const search = new URLSearchParams();
  for (const [key, value] of Object.entries(params)) {
    if (value === undefined || value === null || value === '') {
      continue;
    }
    search.set(key, String(value));
  }
  const query = search.toString();
  return query ? `?${query}` : '';
}

/** Публичный интерфейс клиента (используется сторами и экранами). */
export interface TensaClient {
  readonly baseUrl: () => string;
  health(): Promise<HealthStatus>;
  version(): Promise<string>;
  listNotes(params?: { limit?: number; offset?: number }): Promise<Note[]>;
  createNote(note: NewNote): Promise<Note>;
  getNote(id: string): Promise<Note>;
  updateNote(id: string, note: NewNote): Promise<Note>;
  deleteNote(id: string): Promise<void>;
  searchNotes(query: string, limit?: number): Promise<Note[]>;
  listApprovals(): Promise<Action[]>;
  getApproval(id: string): Promise<Action>;
  approve(id: string, approvedBy?: string): Promise<Action>;
  reject(id: string, rejectedBy?: string): Promise<Action>;
  listNodes(params?: { onlineOnly?: boolean }): Promise<NodeInfo[]>;
  query(text: string, options?: { useCloud?: boolean; model?: string }): Promise<QueryResponse>;
}

/**
 * Создать клиент API.
 *
 * Разбор ответа: 204 → `undefined`; JSON → значение; иначе текст как сообщение
 * ошибки. Тело `{ error }` превращается в {@link ApiError} с этим текстом.
 */
export function createTensaClient(options: TensaClientOptions = {}): TensaClient {
  const readBaseUrl = (): string =>
    normalizeBaseUrl(typeof options.baseUrl === 'function' ? options.baseUrl() : options.baseUrl);
  const readToken = (): string | null => {
    const value = typeof options.token === 'function' ? options.token() : options.token;
    return value ? String(value) : null;
  };
  const doFetch: typeof fetch = options.fetchImpl ?? ((input, init) => fetch(input, init));

  async function request<T>(
    method: string,
    path: string,
    params: Record<string, string | number | boolean | undefined | null> = {},
    body?: unknown,
  ): Promise<T> {
    const url = `${readBaseUrl()}${path}${buildQuery(params)}`;
    const headers: Record<string, string> = { Accept: 'application/json' };
    const token = readToken();
    if (token) {
      headers.Authorization = `Bearer ${token}`;
    }
    if (body !== undefined) {
      headers['Content-Type'] = 'application/json';
    }

    let response: Response;
    try {
      response = await doFetch(url, {
        method,
        headers,
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch (error) {
      // Сеть: сервер выключен, Tailscale отвалился — статуса нет.
      const reason = error instanceof Error ? error.message : String(error);
      throw new ApiError(0, reason, null);
    }

    if (!response.ok) {
      const payload = await readBody(response);
      if (response.status === 401 && options.onUnauthorized) {
        options.onUnauthorized();
      }
      const message =
        payload && typeof payload === 'object' && typeof (payload as ApiErrorBody).error === 'string'
          ? (payload as ApiErrorBody).error!
          : `${response.status} ${response.statusText}`;
      throw new ApiError(response.status, message, payload);
    }

    if (response.status === 204) {
      return undefined as T;
    }
    return (await readBody(response)) as T;
  }

  return {
    baseUrl: readBaseUrl,

    health: () => request<HealthStatus>('GET', '/health'),
    version: () => request<string>('GET', '/version'),

    listNotes: ({ limit = 50, offset = 0 } = {}) =>
      request<Note[]>('GET', '/notes', { limit, offset }),
    createNote: (note) => request<Note>('POST', '/notes', {}, normalizeNote(note)),
    getNote: (id) => request<Note>('GET', `/notes/${encodeURIComponent(id)}`),
    updateNote: (id, note) =>
      request<Note>('PUT', `/notes/${encodeURIComponent(id)}`, {}, normalizeNote(note)),
    deleteNote: async (id) => {
      await request<void>('DELETE', `/notes/${encodeURIComponent(id)}`);
    },
    searchNotes: (query, limit = 10) => request<Note[]>('GET', '/notes/search', { q: query, limit }),

    listApprovals: () => request<Action[]>('GET', '/approvals'),
    getApproval: (id) => request<Action>('GET', `/approvals/${encodeURIComponent(id)}`),
    approve: (id, approvedBy) =>
      request<Action>(
        'POST',
        `/approvals/${encodeURIComponent(id)}/approve`,
        {},
        approvedBy ? { approved_by: approvedBy } : {},
      ),
    reject: (id, rejectedBy) =>
      request<Action>(
        'POST',
        `/approvals/${encodeURIComponent(id)}/reject`,
        {},
        rejectedBy ? { rejected_by: rejectedBy } : {},
      ),

    // Маршрут есть в контракте (Контракт 9), но MVP-бэкенд отвечает 501.
    listNodes: ({ onlineOnly = false } = {}) =>
      request<NodeInfo[]>('GET', '/nodes', { online_only: onlineOnly ? 'true' : undefined }),

    query: (text, options = {}) =>
      request<QueryResponse>('POST', '/query', {}, {
        text,
        ...(options.useCloud ? { use_cloud: true } : {}),
        ...(options.model ? { model: options.model } : {}),
      }),
  };
}

/** Привести заметку к телу контракта: тело — строка, теги — массив строк. */
function normalizeNote(note: NewNote): NewNote {
  return {
    title: note.title ?? '',
    body: note.body ?? '',
    tags: Array.isArray(note.tags) ? note.tags : [],
  };
}

/** Прочитать тело ответа: JSON, иначе текст; пустое тело → `null`. */
async function readBody(response: Response): Promise<unknown> {
  const text = await response.text();
  if (!text) {
    return null;
  }
  const contentType = response.headers.get('content-type') ?? '';
  if (contentType.includes('json')) {
    try {
      return JSON.parse(text);
    } catch {
      return text;
    }
  }
  return text;
}
