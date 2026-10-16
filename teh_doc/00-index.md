# Tensa — Полная техническая документация

**Версия:** 0.1.0
**Дата:** 2026-10-06
**Статус:** Техническая документация завершена

---

## О проекте

**Tensa** — персональный ИИ-секретарь. Работает на нескольких узлах (десктоп, Android), синхронизируется, использует локальную и облачную LLM, работает с документами, почтой, календарём, Telegram, приложениями.

**Ключевые принципы:**
- Privacy-first (локальные модели, Tailscale, без облака Tensa).
- Безопасность (ApprovalStore, белый список приложений, аудит).
- Кроссплатформенность (Windows, Linux, Android; macOS, iOS — после MVP).
- Open-source (текущий этап), с планами коммерциализации.
- Разработка силами AI-ассистента (GLM 5.3) под архитектурным руководством.

---

## Структура документации

Документация разделена на 4 части:

| Файл | Содержание | Статус |
|---|---|---|
| `00-index.md` | Оглавление (этот файл) | ✅ |
| `01-adr.md` | 21 Architecture Decision Records | ✅ |
| `02-contracts.md` | 13 контрактов (SQL, Lance, TOML, Protobuf, UDL, OpenAPI) | ✅ |
| `03-specs.md` | 14 технических заданий (11 крейтов + 3 приложения) | ✅ |
| `04-examples.md` | Примеры кода и референсы | ✅ |

---

## 01-adr.md — Architecture Decision Records (21)

| № | ADR | Тема |
|---|---|---|
| 001 | Rust как язык ядра | Фундаментальное решение |
| 002 | Rust + внешние сервисы | Сетевые операции |
| 003 | FFI к llama.cpp | Локальный инференс |
| 004 | Lance для векторного поиска | Память |
| 005 | fastembed-rs для эмбеддингов | Эмбеддинги |
| 006 | uniffi для FFI-мостов | FFI |
| 007 | Cargo workspace + CI | Сборка |
| 008 | iOS вне MVP | Платформы |
| 009 | macOS вне MVP | Платформы |
| 010 | PWA как временный интерфейс | iOS |
| 011 | Syncthing как библиотека | Синхронизация файлов |
| 012 | gRPC (tonic) для синхронизации | Протокол |
| 013 | On-demand синхронизация | Данные |
| 014 | Фоновые задачи на Android | Android |
| 015 | Почта напрямую, несколько ящиков | Почта |
| 016 | ПК постоянно включён, WoL отложен | Инфраструктура |
| 017 | Запуск приложений | Инструменты |
| 018 | Синхронизация файлов через Syncthing | Файлы |
| 019 | OCR-движок (RustO!) | OCR |
| 020 | Документы (PDF, DOCX, XLSX, PPTX) | Документы |
| 021 | Иерархическая память (Letta-style) | Память |

---

## 02-contracts.md — Контракты (13)

| № | Контракт | Формат | Расположение |
|---|---|---|---|
| 1 | ApprovalStore | SQL (SQLite) | `crates/tensa-approval/schema.sql` |
| 2 | Память | Lance | `crates/tensa-memory/schema.rs` |
| 3 | Заметки/контакты | SQL (SQLite) | `crates/tensa-core/schema.sql` |
| 4 | Конфигурация | TOML | `crates/tensa-core/config.rs` |
| 5 | Белый список приложений | TOML | `crates/tensa-tools/apps/apps.toml` |
| 6 | Учётные записи почты | TOML | `crates/tensa-tools/email/accounts.toml` |
| 7 | Реестр узлов | SQL (SQLite) | `crates/tensa-nodes/schema.sql` |
| 8 | gRPC-контракты | Protobuf | `docs/contracts/proto/` |
| 9 | uniffi UDL | UDL | `docs/contracts/ffi/tensa.udl` |
| 10 | Tauri IPC | Rust + JSON | `apps/desktop/src-tauri/src/commands.rs` |
| 11 | Flutter MethodChannel | Kotlin + Dart | `apps/android/lib/api/tensa.dart` |
| 12 | PWA HTTP API | OpenAPI | `docs/contracts/pwa/openapi.yaml` |
| 13 | Core Memory | SQL (SQLite) | `crates/tensa-memory/schema.sql` |

---

## 03-specs.md — Технические задания (14)

### Крейты (11)

| № | Крейт | Назначение |
|---|---|---|
| 1 | `tensa-approval` | ApprovalStore (центральный элемент безопасности) |
| 2 | `tensa-sync` | Синхронизация (gRPC, Syncthing) |
| 3 | `tensa-router` | Маршрутизация LLM (локальная/облачная) |
| 4 | `tensa-memory` | Память (Lance, fastembed-rs) |
| 5 | `tensa-nodes` | Реестр узлов (heartbeat, роли, соединения) |
| 6 | `tensa-tools` | Инструменты (Telegram, Email, Calendar, Apps, Documents, OCR) |
| 7 | `tensa-llm` | Локальный инференс (llama.cpp FFI) |
| 8 | `tensa-perf` | Профилирование устройства, роли |
| 9 | `tensa-ffi` | FFI-биндинги (uniffi) |
| 10 | `tensa-core` | Координатор (Config, Notes, Contacts) |
| 11 | `tensa-cli` | CLI для отладки |

### Приложения (3)

| № | Приложение | Стек | Платформы |
|---|---|---|---|
| 12 | `apps/desktop` | Tauri v2 + Svelte 5 | Windows, Linux |
| 13 | `apps/android` | Flutter + Kotlin | Android |
| 14 | `apps/pwa` | Svelte | iOS (Safari), fallback |

---

## 04-examples.md — Примеры кода

| Пример | Что показывает |
|---|---|
| Telegram-бот | Архитектура «Telegram → Handler → DB Manager» |
| PDF-детект | `is_scanned_pdf()` — эвристика определения сканов |
| Debounce | 500ms debounce для проверки соединения |
| LLM-тесты | Tool use, reasoning, сложный сценарий на Gemma 4 E2B |
| Плавающий виджет | Концепция виджета (Т + ...) |
| Референсы | Что смотрели (OCR, PDF, DOCX, XLSX, LLM, модели, библиотеки) |

---

## Ключевые технические решения

### Язык и архитектура
- **Rust** — язык ядра (ADR-001).
- **Cargo workspace** — структура проекта (ADR-007).
- **Rust + внешние сервисы** — сетевые операции (ADR-002).
- **gRPC (tonic)** — синхронизация между узлами (ADR-012).

### LLM
- **Локальная:** TheStageAI/gemma-4-E2B-it-GGUF (XS/S/M) через llama.cpp FFI (ADR-003).
- **Облачная:** google/gemma-4-26b-a4b-it (free/paid) через OpenRouter.
- **Fallback:** бесплатная → платная → локальная → degraded mode.
- **Бюджет:** 100K токенов/день, $10/мес.

### Память и поиск
- **Lance 13.0** — векторный поиск (ADR-004).
- **fastembed-rs 7.1** — эмбеддинги (ADR-005).
- **multilingual-e5-small** — модель (384 dim).
- **Чанкинг:** 1000 символов, перекрытие 200.
- **Индекс:** Flat.
- **Иерархическая память** — core / recall / archival (ADR-021).

### Документы и OCR
- **PDF:** pdfplumber-rs (Apache-2.0).
- **DOCX:** docx-rs.
- **XLSX:** calamine.
- **PPTX:** свой на zip + quick-xml.
- **OCR:** RustO! (MIT) + Cyrillic OCR.
- **Определение сканов:** постранично, 3 правила.

### Синхронизация
- **ApprovalStore:** постоянно, через gRPC.
- **Память/заметки/контакты:** on-demand, через gRPC.
- **Файлы:** Syncthing (в дистрибутиве).
- **Конфликты:** LWW + векторные часы.

### Безопасность
- **ApprovalStore** — все действия через одобрение.
- **Белый список приложений** — настраиваемый, сканирование системы.
- **Чёрный список** — опасные утилиты.
- **Запрещённые аргументы** — защита от инъекций.
- **Аудит** — все действия логируются.

### Платформы
- **MVP:** Windows, Linux, Android.
- **После MVP:** macOS, iOS.
- **PWA:** временный интерфейс для iOS.
- **Точки входа:** Rust компилируется под macOS/iOS в CI.

### UI
- **Десктоп:** Tauri + Svelte.
- **Android:** Flutter + Kotlin (uniffi).
- **PWA:** Svelte.
- **Плавающий виджет:** две зоны (Т + ...), overlay + App Widget (Android), отдельное окно (ПК).

---

## Структура проекта

```
tensa/
├── Cargo.toml                    # workspace
├── Cargo.lock
├── crates/
│   ├── tensa-core/
│   ├── tensa-approval/
│   ├── tensa-sync/
│   ├── tensa-router/
│   ├── tensa-memory/
│   ├── tensa-nodes/
│   ├── tensa-tools/
│   ├── tensa-llm/
│   ├── tensa-perf/
│   ├── tensa-ffi/
│   └── tensa-cli/
├── apps/
│   ├── desktop/
│   ├── android/
│   └── pwa/
├── docs/
│   ├── 00-index.md
│   ├── 01-adr.md
│   ├── 02-contracts.md
│   ├── 03-specs.md
│   ├── 04-examples.md
│   ├── contracts/
│   │   ├── proto/
│   │   ├── ffi/
│   │   ├── pwa/
│   │   └── schemas/
│   └── guides/
└── .github/
    └── workflows/
```

---

## Дорожная карта

### MVP (v0.1)
- Windows, Linux, Android.
- Локальная LLM (Gemma 4 E2B) + облачная (Gemma 4 26B A4B).
- ApprovalStore + Telegram-бот + почта (IMAP/SMTP).
- Поиск по файлам (PDF, DOCX, XLSX, PPTX).
- Календарь (CalDAV).
- Запуск приложений.
- Синхронизация десктоп ↔ Android (базовая).
- Плавающий виджет.
- PWA для iOS.
- Иерархическая память (core / recall / archival).

### После MVP (v0.2+)
- macOS.
- iOS (нативное приложение).
- WhatsApp Cloud API.
- WoL.
- Несколько почтовых ящиков.
- Шифрование at rest.
- Remote wipe.
- GPU-ускорение.
- IVF-PQ, HNSW.
- Sleep-time compute.
- Синхронизация памяти между узлами.

### Долгосрочно
- Коммерциализация.
- Федеративная модель (у каждого свой экземпляр + общий шлюз).
- Мультипользовательская поддержка.
- Расширение на другие платформы.

---

## Следующие шаги

1. **Продуктовая документация** (Notion/Confluence) — описание продукта, аудитория, бизнес-модель, roadmap.
2. **Руководства** (`docs/guides/`) — always_on_setup, tailscale_guide, ios_install, whatsapp_setup, sysadmin_guide, wol_setup.
3. **Документация для GLM 5.3** — как формулировать задачи, проверять результат, ревьюить.
4. **Начало разработки** — по ТЗ, начиная с `tensa-approval`.

---

## Связанные документы

- `01-adr.md` — Architecture Decision Records.
- `02-contracts.md` — Контракты.
- `03-specs.md` — Технические задания.
- `04-examples.md` — Примеры кода.
