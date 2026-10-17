# Tensa

[![CI](https://github.com/kostyuk230378-cmd/TENSA/actions/workflows/ci.yml/badge.svg)](https://github.com/kostyuk230378-cmd/TENSA/actions/workflows/ci.yml)
[![License: MIT OR Apache-2.0](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](#лицензия)

**Персональный ИИ-секретарь, который работает на ваших устройствах и делает только то, что вы одобрили.**

Tensa — self-hosted секретарь для одного человека: документы, почта, календарь, заметки,
контакты. Модель — локальная (llama.cpp, GGUF) или облачная (OpenRouter); всё, что меняет мир
за пределами приложения — отправить письмо, поставить встречу, запустить программу — проходит
через `ApprovalStore`: сначала одобрение владельца, потом выполнение, с аудитом.

Ядро — Rust-воркспейс (14 членов: ядро и обёртки llama.cpp). Клиенты: **PWA** (Svelte 5),
**Android** (нативный клиент, APK) и **Windows-установщик** (ядро + CLI + Telegram-бот + PWA).
Синхронизация узлов (gRPC) — в разработке.

---

## Что уже работает

| Контур | Возможности |
|---|---|
| **HTTP-ядро** | `/health`, `/version`, заметки (CRUD + FTS5-поиск), очередь одобрений (approve/reject), SSE `/events`, календарь (guard конфликтов + ICS), почтовые аккаунты, `POST /query` (облако/локальная модель с fallback), приём модели эмбеддингов из APK |
| **Память** | иерархическая память (core/recall/archival, ADR-021), семантический поиск на `intfloat/multilingual-e5-small`: `tensa memory index / search / list / delete` (проверено на реальной модели) |
| **Инструменты** | Telegram-бот «@your_tensa_bot» (`/status`, `/approvals`, `/approve`, `/reject`, inline-кнопки, сторож очереди), email через SMTP (lettre, STARTTLS/implicit TLS), документы PDF/DOCX/PPTX/XLSX/TXT/MD, Wake-on-LAN |
| **PWA** | Чат (через ядро), Статус, Одобрения, Заметки, Поиск, Настройки — Svelte 5, SSE, i18n ru/en, PWA-манифест, офлайн-оболочка |
| **Android (APK)** | Чат через ядро (резерв — прямой OpenRouter), Одобрения, Заметки, Поиск по заметкам, Почта (аккаунты), Календарь (список + создание, 409-конфликт), Статус, Настройки; кнопка **«Установить модель»** — передаёт e5-small из assets в ядро |
| **Windows** | `TensaSetup.exe`: ядро + CLI + Telegram-бот + PWA + **модель e5-small в комплекте**; ярлыки, деинсталлятор; ставится без админа в `%LOCALAPPDATA%\Tensa` |

## Модели

- **Эмбеддер памяти e5-small** (`intfloat/multilingual-e5-small`) — **в комплекте** установщика
  Windows и APK; работает офлайн, сеть не нужна.
- **Локальная LLM** — GGUF в `~/.tensa/models` (по умолчанию `gemma-4-E2B-it-…`; ярлык
  «Tensa (download models)» качает публичную сборку). Локальный инференс — llama.cpp внутри ядра.
- **Большие модели** — OpenRouter: ключ в `~/.tensa/secrets.toml` (`[openrouter] credential = "sk-or-…"`)
  или `OPENROUTER_API_KEY`; чат PWA/APK идёт через ядро, ключ остаётся на ПК.
  Ключ выдаётся на https://openrouter.ai/keys.

## Быстрый старт

1. Скачайте `TensaSetup.exe` из [Releases](https://github.com/kostyuk230378-cmd/TENSA/releases) и установите.
2. Запустите ярлык **Tensa** — откроется браузер на `http://127.0.0.1:8765/app`
   (окно консоли держит сервер; закрыли — сервер остановлен).
3. Токен доступа — файл `%USERPROFILE%\.tensa\pwa_token`; вставьте его на экране подключения PWA.
4. Телефон: установите APK (Release или `http://<ip-пк>:8765/static/Tensa.apk`), в Настройках
   укажите адрес ядра (например `http://<ip-пк>:8765`) и тот же токен.
5. Проверка памяти на ПК:
   `tensa-cli.exe -c %USERPROFILE%\.tensa\config.toml memory index <файл>` →
   `tensa-cli.exe … memory search "запрос"`.


## Скачать

Актуальный релиз — **[v0.2.0](https://github.com/kostyuk230378-cmd/TENSA/releases/tag/v0.2.0)**:

| Платформа | Файл | Размер |
|---|---|---|
| Windows | [TensaSetup.exe](https://github.com/kostyuk230378-cmd/TENSA/releases/download/v0.2.0/TensaSetup.exe) | 214 МБ |
| Android | [Tensa.apk](https://github.com/kostyuk230378-cmd/TENSA/releases/download/v0.2.0/Tensa.apk) | 239 МБ |

## Скриншоты

![Чат через ядро](https://raw.githubusercontent.com/kostyuk230378-cmd/TENSA/main/pwa/screenshots/07-chat.png)
![Статус ядра](https://raw.githubusercontent.com/kostyuk230378-cmd/TENSA/main/pwa/screenshots/02-status.png)

## Тесты и качество

- `cargo test --workspace` — **356 passed, 0 failed, 1 ignored** (48 тестовых бинарей);
- feature-прогоны: `tensa-memory --features lance,fastembed` — **35 passed** (1 ignored),
  `tensa-llama --features native` — **36 passed** (2 ignored — требуют реальную GGUF-модель);
- `cargo clippy --workspace --all-targets -- -D warnings` — 0 предупреждений; `cargo fmt --check` — чисто;
- MSRV — 1.95; CI (GitHub Actions): fmt + clippy + tests, MSRV-проверка, memory-job (Lance + fastembed) — зелёные.

## Документация

- `teh_doc/` — техническая документация: 21 ADR, 13 контрактов, 14 ТЗ, `FIN.md` (финальная сводка);
- `START_HERE.md` — инструкция AI-разработчику проекта;
- `PRODUCT DOC/` — продуктовая документация (PD_1–7), `Guide/` — руководства пользователя;
- `README_DOCS.md` — прежний индекс документации.

## Лицензия

MIT OR Apache-2.0.
