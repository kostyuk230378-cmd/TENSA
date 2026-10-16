/**
 * Service Worker PWA Tensa (ТЗ apps/pwa).
 *
 * Задачи: кэшировать статику (оболочку и ассеты) и давать ограниченный
 * offline — при отсутствии сети навигация отдаёт закэшированную оболочку.
 *
 * Чего здесь принципиально нет (ограничения ТЗ и ядра):
 * - фоновых задач и push (PWA не получает уведомления без пользователя);
 * - кэширования API: запросы к `/api/v1/**` всегда идут в сеть, иначе
 *   интерфейс показывал бы устаревшие одобрения/заметки;
 * - «магической» авторизации — токен живёт в localStorage страницы.
 *
 * Обновление: при выпуске новой сборки меняется VERSION_CACHE — старые
 * кэши удаляются на активации.
 */
const VERSION_CACHE = 'tensa-static-v1';

/** Оболочка, доступная офлайн сразу после установки. */
const PRECACHE_URLS = [
  '/',
  '/app',
  '/static/manifest.json',
  '/static/icons/icon-192.png',
  '/static/icons/icon-512.png',
];

self.addEventListener('install', (event) => {
  event.waitUntil(
    caches
      .open(VERSION_CACHE)
      .then((cache) => cache.addAll(PRECACHE_URLS))
      .then(() => self.skipWaiting()),
  );
});

self.addEventListener('activate', (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) =>
        Promise.all(keys.filter((key) => key !== VERSION_CACHE).map((key) => caches.delete(key))),
      )
      .then(() => self.clients.claim()),
  );
});

self.addEventListener('fetch', (event) => {
  const request = event.request;
  const url = new URL(request.url);

  // Только собственные GET-запросы; API — всегда сеть.
  if (request.method !== 'GET' || url.origin !== self.location.origin) {
    return;
  }
  if (url.pathname.startsWith('/api/')) {
    return;
  }

  event.respondWith(
    caches.match(request).then(
      (cached) =>
        cached ??
        fetch(request)
          .then((response) => {
            // Кладём в кэш только успешные ответы (и не opaque).
            if (response.ok && response.type === 'basic') {
              const copy = response.clone();
              caches.open(VERSION_CACHE).then((cache) => cache.put(request, copy));
            }
            return response;
          })
          .catch(() => caches.match('/app')),
    ),
  );
});
