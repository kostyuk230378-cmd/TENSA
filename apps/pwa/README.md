# Tensa PWA (apps/pwa)

PWA-версия Tensa на Svelte (ТЗ `apps/pwa`, ADR-010): временный интерфейс для
iOS и fallback для других устройств. Работает с ядром по HTTP (Контракт 12).

## Раскладка

```
apps/pwa/            исходники (Vite + Svelte)
├── src/             main.js, App.svelte, lib/{api,stores,i18n,components}, routes/
├── static/          manifest.json, sw.js, icons/ (копируются в сборку как есть)
├── scripts/         make_icons.py — перегенерация иконок (Pillow)
└── tests/unit/      vitest-тесты API-клиента
pwa/static/          АРТЕФАКТ сборки (его раздаёт tensa-server по /static/...)
```

[derived]-маппинг: ТЗ перечисляет `apps/pwa/static/` в исходниках и размещение
сборки «в core/static/pwa/»; здесь исходники лежат в `apps/pwa/`, а сборка
идёт в `pwa/static/` — именно этот каталог `tensa-server` отдаёт по умолчанию
(`DEFAULT_STATIC_DIR = "pwa/static"`).

## Сборка и запуск

```powershell
cd apps/pwa
npm install          # один раз
npm run build        # артефакт → ../../pwa/static
npm run test         # vitest (unit API-клиента)
```

Запуск с ядром (из корня репозитория):

```powershell
cargo run -p tensa-server -- C:\path\to\config.toml
# сервер: http://127.0.0.1:8765/app — статика PWA; /api/v1 — API
```

Токен доступа: файл `pwa_token` в корне данных (каталог `config.toml`);
ядро создаёт его при первом запуске. В PWA токен вводится на экране
подключения и хранится в localStorage.

## Экраны (MVP)

Рабочие: подключение (токен), статус (+SSE `status:health`), одобрения
(approve/reject), заметки (создание/удаление), поиск (FTS5), узлы, настройки
(адрес API, токен, язык ru/en).

Честные заглушки («этап …»): документы, почта, календарь, приложения, голос.

## Особенности

- SSE читается через `fetch`-стрим с заголовком авторизации: браузерный
  `EventSource` не умеет заголовки, а API закрыт Bearer-токеном.
- Service Worker кэширует только статику; `/api/**` никогда не кэшируется.
- `npm run dev` проксирует `/api` на `http://127.0.0.1:8765` (см. vite.config.js).

## Демо-скриншоты

`pwa/scripts/capture_screens.mjs` (headless Edge + CDP) прогоняет живой сценарий
(токен → статус → одобрения → создание заметки через форму → поиск → настройки)
и сохраняет скриншоты в `pwa/screenshots/`:

```powershell
node pwa/scripts/capture_screens.mjs "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" http://127.0.0.1:8765/app <токен> pwa/screenshots
```

## Дальше

- E2E (Playwright, Chrome + Safari) и Lighthouse — следующий шаг по ТЗ;
- экраны документов/почты/календаря/приложений/голоса — вместе с этапами
  соответствующих инструментов (tensa-tools).
