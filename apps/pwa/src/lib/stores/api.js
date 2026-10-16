/**
 * Данные PWA Tensa: единый API-клиент, стор-состояние и поток событий (SSE).
 *
 * Все экраны читают сторы отсюда; сетевые вызовы — через готовые функции
 * (refresh…, create…, decide): компоненты не знают про fetch и заголовки.
 *
 * Особенности сервера (Контракт 12, MVP-бэкенд tensa-server):
 * - данные авторизуются Bearer-токеном (см. stores/token.js);
 * - SSE-поток `/api/v1/events` читается fetch-стримом с заголовком
 *   (EventSource заголовки не умеет, см. lib/api/events.ts);
 * - часть маршрутов отвечает 501 — экраны показывают честное «ещё нет».
 */
import { get, writable } from 'svelte/store';
import { createTensaClient } from '../api/tensa';
import { createEventStream } from '../api/events';
import { withAuth } from '../api/auth';
import { currentToken } from './token';
import { baseUrl, reportError } from './ui';
import { describeError } from './ui';

/** Состояние ядра (`GET /health`). */
export const health = writable(null);

/** Ожидающие одобрения (`GET /approvals`). */
export const approvals = writable([]);

/** Свежие заметки (`GET /notes`). */
export const notes = writable([]);

/** Результаты последнего поиска по заметкам. */
export const searchResults = writable([]);

/** Реестр узлов (в MVP-бэкенде пока 501). */
export const nodes = writable([]);

/** Состояние SSE-потока: connecting/open/reconnecting/unauthorized/error/closed. */
export const streamState = writable('closed');

/** Единый клиент: адрес и токен читаются перед каждым запросом. */
export const client = createTensaClient({
  baseUrl: () => get(baseUrl),
  token: () => currentToken(),
  onUnauthorized: () => reportError('401: токен не принят сервером — проверьте настройки'),
});

/** Перезагрузить состояние ядра. */
export async function refreshHealth() {
  try {
    health.set(await client.health());
  } catch (value) {
    reportError(value);
  }
}

/** Перезагрузить очередь одобрения. */
export async function refreshApprovals() {
  try {
    approvals.set(await client.listApprovals());
  } catch (value) {
    reportError(value);
  }
}

/** Перезагрузить список заметок. */
export async function refreshNotes() {
  try {
    notes.set(await client.listNotes({ limit: 50, offset: 0 }));
  } catch (value) {
    reportError(value);
  }
}

/** Поиск по заметкам (FTS5 на стороне ядра). */
export async function runSearch(query) {
  try {
    searchResults.set(await client.searchNotes(query, 20));
  } catch (value) {
    reportError(value);
  }
}

/** Создать заметку и добавить её в начало списка. */
export async function createNote(note) {
  const created = await client.createNote(note);
  notes.update((list) => [created, ...list]);
  return created;
}

/** Удалить заметку (мягкое удаление на стороне ядра). */
export async function deleteNote(id) {
  await client.deleteNote(id);
  notes.update((list) => list.filter((note) => note.id !== id));
  searchResults.update((list) => list.filter((note) => note.id !== id));
}

/**
 * Решение по действию: подтвердить (`approve = true`) или отклонить.
 * Решённое действие уходит из очереди ожидания.
 */
export async function decide(id, approve) {
  const action = approve ? await client.approve(id) : await client.reject(id);
  approvals.update((list) => list.filter((item) => item.id !== action.id));
  return action;
}

/** Обновить реестр узлов; 501 — честная пустота (экран покажет пояснение). */
export async function refreshNodes() {
  try {
    nodes.set(await client.listNodes());
    return true;
  } catch (value) {
    nodes.set([]);
    if (!value?.isNotImplemented) {
      reportError(describeError(value));
    }
    return false;
  }
}

/** Открыт ли поток событий (для маршрута 501 узлов). */
export function isNotImplemented(value) {
  return value?.isNotImplemented === true;
}

// ---- SSE: поток событий ядра ----------------------------------------------

let streamHandle = null;

/** Разобрать `status:health` и обновить стор; битый JSON игнорируем. */
function handleEvent(event) {
  if (event.event === 'status:health') {
    try {
      health.set(JSON.parse(event.data));
    } catch {
      // Нечитаемый кадр — не повод ломать интерфейс.
    }
  } else if (event.event === 'approval:new' || event.event === 'approval:updated') {
    void refreshApprovals();
  }
}

/** Открыть поток событий (идемпотентно: прежний закрывается). */
export function startEventStream() {
  stopEventStream();
  streamHandle = createEventStream({
    headers: () => withAuth({ Accept: 'text/event-stream' }, currentToken()),
    onEvent: handleEvent,
    onState: (state, detail) => {
      streamState.set(state);
      if (state === 'unauthorized') {
        reportError('401: токен не принят сервером — проверьте настройки');
      } else if (state === 'error' && detail) {
        // Тихо: состояние видно в StatusBar, баннер не засоряем.
      }
    },
  });
}

/** Закрыть поток событий и остановить переподключения. */
export function stopEventStream() {
  if (streamHandle) {
    streamHandle.stop();
    streamHandle = null;
  }
  streamState.set('closed');
}

/** Полная загрузка при появлении доступа. */
export function boot() {
  void refreshHealth();
  void refreshApprovals();
  void refreshNotes();
  startEventStream();
}
