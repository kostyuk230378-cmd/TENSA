/**
 * UI-состояние PWA Tensa: экран, язык, адрес API, ошибки.
 *
 * Здесь нет данных ядра — только представление (см. stores/api.js).
 * Настройки переживают перезапуск (localStorage); секрет (токен) хранит
 * отдельный стор `stores/token.js`.
 */
import { writable } from 'svelte/store';

/** Ключ localStorage с адресом API. */
const BASE_KEY = 'tensa.pwa.base';

/** Ключ localStorage с языком интерфейса. */
const LANG_KEY = 'tensa.pwa.lang';

/** Прочитать значение из localStorage; недоступен → пустая строка. */
function readStorage(key) {
  try {
    return localStorage.getItem(key) ?? '';
  } catch {
    return '';
  }
}

/** Сохранить значение; пустое — удалить запись. */
function persist(key, value) {
  try {
    if (value) {
      localStorage.setItem(key, value);
    } else {
      localStorage.removeItem(key);
    }
  } catch {
    // Приватный режим: настройки не переживут перезапуск — не критично.
  }
}

/** Адрес API: пусто — тот же origin (`/api/v1`, Контракт 12). */
export const baseUrl = writable(readStorage(BASE_KEY));

/** Язык интерфейса: `ru` (по умолчанию) или `en`. */
export const language = writable(readStorage(LANG_KEY) || 'ru');

/** Текущий экран (см. навигацию в App.svelte). */
export const screen = writable('status');

/** Текст ошибки для баннера сверху; пустая строка — нет ошибки. */
export const error = writable('');

// Изменения настроек сразу попадают в localStorage.
baseUrl.subscribe((value) => persist(BASE_KEY, String(value ?? '').trim()));
language.subscribe((value) => persist(LANG_KEY, value === 'en' ? 'en' : 'ru'));

/** Записать адрес API (без хвостовых пробелов). */
export function setBaseUrl(value) {
  baseUrl.set(String(value ?? '').trim());
}

/** Переключить язык (`en`/`ru`, остальное игнорируется). */
export function setLanguage(value) {
  language.set(value === 'en' ? 'en' : 'ru');
}

/** Показать ошибку в баннере. */
export function reportError(value) {
  error.set(describeError(value));
}

/** Убрать баннер ошибки. */
export function clearError() {
  error.set('');
}

/**
 * Человекочитаемое описание ошибки: ApiError уже несёт текст сервера,
 * прочие значения приводим к строке.
 */
export function describeError(value) {
  if (value === null || value === undefined) {
    return 'неизвестная ошибка';
  }
  if (typeof value === 'string') {
    return value;
  }
  if (typeof value.message === 'string' && value.message) {
    return value.message;
  }
  return String(value);
}
