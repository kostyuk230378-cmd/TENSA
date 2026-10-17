/**
 * Простая i18n PWA Tensa (ТЗ apps/pwa: lib/i18n/ru.json, en.json).
 *
 * `t` — производный стор от языка: компоненты читают переводы как `{$t('key')}`.
 * Неизвестный ключ возвращает переданный запасной текст или сам ключ.
 */
import { derived } from 'svelte/store';
import { language } from '../stores/ui';
import ru from './ru.json';
import en from './en.json';
import * as extra from './extra.js';

/** Словари с дополнительными строками (экран «Чат»). */
const dictionaries = {
  ru: { ...ru, ...extra.ru },
  en: { ...en, ...extra.en },
};

/** Функция перевода для текущего языка. */
export const t = derived(language, (lang) => {
  const dict = dictionaries[lang] ?? dictionaries.ru;
  return (key, fallback) => dict[key] ?? fallback ?? key;
});
