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
| **Файлы на дисках** | локальный индекс всех fixed-дисков (кроме системных каталогов): поиск по имени и содержимому PDF/DOCX/PPTX/XLSX/TXT/MD (FTS5). Запрос «найди договор …» отвечает списком файлов с путями; для остальных вопросов фрагменты подмешиваются в контекст модели |
| **Инструменты** | Telegram-бот «@your_tensa_bot» (`/status`, `/approvals`, `/approve`, `/reject`, inline-кнопки, сторож очереди), email через SMTP (lettre, STARTTLS/implicit TLS), документы PDF/DOCX/PPTX/XLSX/TXT/MD, Wake-on-LAN |
| **PWA** | Чат (через ядро), Статус, Одобрения, Заметки, Поиск, Настройки — Svelte 5, SSE, i18n ru/en, PWA-манифест, офлайн-оболочка |
| **Android (APK)** | Чат через ядро (резерв — прямой OpenRouter), Одобрения, Заметки, Поиск по заметкам, Почта (аккаунты), Календарь (список + создание, 409-конфликт), Статус, Настройки; кнопка **«Установить модель»** — передаёт e5-small из assets в ядро |
| **Windows** | `TensaSetup.exe`: ядро + CLI + Telegram-бот + PWA + **модель e5-small в комплекте; модели речи скачиваются из «Настроек»**; ярлыки, деинсталлятор; ставится без админа в `%LOCALAPPDATA%\Tensa` |
| **Голос** | Кнопка микрофона в виджете: запись → локальный whisper (модель скачивается из «Настроек», офлайн) → текст в поле ввода; ~15–20 с на фразу |
| **Виджет** | Прозрачное always-on-top окно: зелёная пилюля и лампочка **перетаскиваются мышью** (Pointer Events с захватом), разворот в HUD-панель |

## Модели

- **Эмбеддер памяти e5-small** (`intfloat/multilingual-e5-small`) — **в комплекте** установщика
  Windows и APK; работает офлайн, сеть не нужна.
- **Локальная LLM** — проектный квант [`TheStageAI/gemma-4-E2B-it-GGUF`](https://huggingface.co/TheStageAI/gemma-4-E2B-it-GGUF):
  **`gemma-4-E2B-it-XS-TS-Q3_K_S.gguf`** (~1.73 ГБ, самый лёгкий по памяти) в
  `~/.tensa/models`; модель скачивается одной кнопкой из «Настроек», SHA256 проверяется автоматически.
  Локальный инференс — llama.cpp внутри ядра.
- **Модели распознавания речи** — Whisper tiny/small q5_1/small + Nemotron/Parakeet/Canary (75–797 МБ) скачиваются из «Настроек»; small/Nemotron/Parakeet/Canary — с blob.handy.computer (Handy), tiny/small q5_1 — с Hugging Face whisper.cpp. Nemotron/Parakeet/Canary работают через отдельный runtime 	ensa-asr.exe (входит в установщик).
- **Модель ассистента** — локальная Gemma 4 E2B (~1.65 ГБ, скачивание из «Настроек») или OpenRouter google/gemma-4-26b-a4b-it (API-ключ вводится там же).
- **Большие модели** — OpenRouter: ключ в `~/.tensa/secrets.toml` (`[openrouter] credential = "sk-or-…"`)
  или `OPENROUTER_API_KEY`; чат PWA/APK идёт через ядро, ключ остаётся на ПК.
  Ключ выдаётся на https://openrouter.ai/keys.

## Быстрый старт

1. Скачайте `TensaSetup.exe` из [Releases](https://github.com/kostyuk230378-cmd/TENSA/releases) и установите.
2. Запустите ярлык **Tensa** — откроется браузер на `http://127.0.0.1:8765/app`
   (окно консоли держит сервер; закрыли — сервер остановлен).
3. На этом ПК вход **без токена** (loopback доверенный). Токен — только для телефона:
   `%USERPROFILE%\.tensa\pwa_token`.
4. Телефон: установите APK (Release или `http://<ip-пк>:8765/static/Tensa.apk`), в Настройках
   укажите адрес ядра (например `http://<ip-пк>:8765`) и тот же токен.
5. Проверка памяти на ПК:
   `tensa-cli.exe -c %USERPROFILE%\.tensa\config.toml memory index <файл>` →
   `tensa-cli.exe … memory search "запрос"`.


## Скачать

Актуальный релиз — **[v0.2.8](https://github.com/kostyuk230378-cmd/TENSA/releases/tag/v0.2.8)**:

| Платформа | Файл | Размер |
|---|---|---|
| Windows | [TensaSetup.exe](https://github.com/kostyuk230378-cmd/TENSA/releases/download/v0.2.8/TensaSetup.exe) | 220 МБ |
| Android | [Tensa.apk](https://github.com/kostyuk230378-cmd/TENSA/releases/download/v0.2.8/Tensa.apk) | 239 МБ |

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
