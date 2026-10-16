/**
 * Значение доступа к API (Контракт 12, `security: BearerAuth`).
 *
 * Пользователь вводит значение на экране первого запуска или в настройках;
 * оно лежит в localStorage браузера и в коде не содержится. Сам заголовок
 * собирает `lib/api/auth.js` — здесь хранится только значение.
 */
import { get, writable } from 'svelte/store';

/** Ключ localStorage со значением доступа. */
const STORAGE_KEY = 'tensa.pwa.access';

/** Прочитать сохранённое значение; недоступный localStorage → пустая строка. */
function read() {
  try {
    return localStorage.getItem(STORAGE_KEY) ?? '';
  } catch {
    return '';
  }
}

/** Сохранить значение; пустая строка убирает запись. */
function persist(value) {
  try {
    if (value) {
      localStorage.setItem(STORAGE_KEY, value);
    } else {
      localStorage.removeItem(STORAGE_KEY);
    }
  } catch {
    // Приватный режим браузера: настройки не переживут перезапуск.
  }
}

/** Текущее значение (пустая строка — не задано). */
export const token = writable(read());

/** Записать новое значение: в хранилище и в localStorage. */
export function setToken(value) {
  const next = (value ?? '').trim();
  token.set(next);
  persist(next);
}

/** Значение без подписки — для сборки заголовков запроса. */
export function currentToken() {
  return get(token);
}

// Любое изменение (в том числе через setToken) сразу попадает в localStorage.
token.subscribe((value) => persist(String(value ?? '')));
