/**
 * Точка входа PWA Tensa (ТЗ apps/pwa): монтирование Svelte-приложения
 * и регистрация Service Worker.
 *
 * Оболочка публична (без токена), данные API — за Bearer-токеном:
 * браузер не прикладывает заголовок при навигации, поэтому вход в интерфейс
 * возможен всегда, а токен проверяется на каждом запросе к `/api/v1`.
 */
import { mount } from 'svelte';
import App from './App.svelte';
import './app.css';

const target = document.getElementById('app');
if (target) {
  mount(App, { target });
}

// Service Worker: кэш статики и ограниченный offline (ТЗ apps/pwa).
// Регистрация не критична: ошибку тихо игнорируем (старый браузер, http).
if ('serviceWorker' in navigator) {
  window.addEventListener('load', () => {
    navigator.serviceWorker.register('/static/sw.js').catch(() => {
      // Офлайн-режим недоступен — интерфейс работает как обычно.
    });
  });
}
