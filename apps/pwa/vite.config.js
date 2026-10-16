// Vite-конфигурация PWA Tensa (ТЗ apps/pwa).
//
// Раскладка сборки (derived-маппинг из ТЗ):
//   исходники   apps/pwa/          (этот каталог — корень Vite)
//   артефакт    pwa/static/        (build.outDir) — каталог раздаёт tensa-server
//
// Почему base = '/static/': сервер отдаёт собранные файлы через ServeDir,
// смонтированный на '/static' (crates/tensa-server/src/routes/static_files.rs),
// а index.html — по '/' и '/app'. Поэтому ссылки на ассеты должны вести
// в '/static/...', иначе браузер получит 501 на корневых путях.
import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

export default defineConfig({
  plugins: [svelte()],

  // Префикс URL ассетов: статику раздаёт tensa-server по /static/...
  base: '/static/',

  // Манифест PWA, service worker и иконки лежат в исходниках рядом с src/ —
  // Vite копирует содержимое publicDir в корень outDir как есть.
  publicDir: 'static',

  build: {
    // Собранный фронтенд кладём прямо в каталог, который раздаёт tensa-server.
    outDir: '../../pwa/static',
    // outDir вне корня проекта: пустой каталог чистим осознанно, иначе
    // предыдущая сборка останется вперемешку с новой.
    emptyOutDir: true,
    target: 'es2022',
  },

  server: {
    port: 5173,
    // Режим разработки: API уходит на локальный tensa-server (порт 8765),
    // поэтому в браузере остаётся same-origin и не нужен CORS.
    proxy: {
      '/api': {
        target: 'http://127.0.0.1:8765',
        changeOrigin: true,
      },
    },
  },

  test: {
    // Unit-тесты API-клиента: чистый fetch, DOM не нужен.
    environment: 'node',
    include: ['tests/unit/**/*.test.js'],
  },
});
