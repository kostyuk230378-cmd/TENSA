# Tensa — Финальная сводка

**Дата:** 2026-10-15
**Версия:** 0.1.0
**Статус:** Техническая документация завершена, разработка в процессе

---

## Что собрано

### 1. Техническая документация

| Файл | Содержание | Статус |
|---|---|---|
| `00-index.md` | Оглавление, структура, дорожная карта | ✅ |
| `01-adr.md` | 21 Architecture Decision Records | ✅ |
| `02-contracts.md` | 13 контрактов | ✅ |
| `03-specs.md` | 14 технических заданий | ✅ |
| `04-examples.md` | Примеры кода и референсы | ✅ |
| `ADR-021.md` | Иерархическая память (отдельный файл) | ✅ |
| `Contract_13.md` | Core Memory (отдельный файл) | ✅ |
| `dop_tz.md` | Дополнение к ТЗ tensa-memory | ✅ |
| `dop_tz_core.md` | Дополнение к ТЗ tensa-core | ✅ |
| `FIN.md` | Финальная сводка (этот файл) | ✅ |

### 2. Руководство для GLM 5.3 (`glm/`)

| Файл | Содержание | Статус |
|---|---|---|
| `00-overview.md` | Обзор для GLM 5.3 | ✅ |
| `01-task-format.md` | Формат задач | ✅ |
| `02-testing.md` | Тестирование | ✅ |
| `03-review.md` | Ревью | ✅ |
| `04-documentation.md` | Документирование | ✅ |
| `05-rollback.md` | Откат | ✅ |
| `06-cargo-workspace.md` | Cargo workspace | ✅ |
| `07-ci.md` | CI/CD | ✅ |

### 3. Системный промпт

| Файл | Содержание | Статус |
|---|---|---|
| `system_prompt_tensa.md` | Системный промпт для LLM Tensa (Gemma 4 E2B / 26B / 31B) | ✅ |

### 4. Продуктовая документация (`PRODUCT DOC/`)

| Часть | Содержание | Статус |
|---|---|---|
| PD_1 | Описание продукта | ✅ |
| PD_2 | Целевая аудитория | ✅ |
| PD_3 | Бизнес-модель | ✅ |
| PD_4 | Roadmap | ✅ |
| PD_5 | Память и персона | ✅ |
| PD_6 | Маркетинговые материалы | ✅ |
| PD_7 | Пользовательские руководства | ✅ |

### 5. Руководства (`Guide/`)

| Файл | Содержание | Статус |
|---|---|---|
| `always_on_setup.md` | Настройка ПК для постоянной работы | ✅ |
| `tailscale_guide.md` | Настройка Tailscale | ✅ |
| `ios_install.md` | Установка на iOS (PWA) | ✅ |
| `whatsapp_setup.md` | Настройка WhatsApp Cloud API | ✅ |
| `sysadmin_guide.md` | Установка в корпоративной сети | ✅ |
| `wol_setup.md` | Настройка WoL (v0.2+) | ✅ |

---

## Итого

| Категория | Количество |
|---|---|
| ADR | **21** |
| Контракты | **13** |
| ТЗ (крейты + приложения) | **14** |
| Дополнения к ТЗ | **2** |
| Файлы руководства GLM 5.3 | **8** |
| Системный промпт | **1** |
| Части продуктовой документации | **7** |
| Руководства | **6** |
| **Всего документов** | **~72** |

---

## Статус разработки

| Крейт | Статус | Тесты |
|---|---|---|
| `tensa-approval` | ✅ Готов | 15 |
| `tensa-sync` | ✅ Готов | 17 |
| `tensa-core` | ✅ Готов | 34 |
| `tensa-memory` | ✅ Готов | 31 |
| `tensa-nodes` | ✅ Готов | 24 |
| `tensa-llm` | ⚠️ Готов без нативного слоя | 30 |
| `tensa-perf` | 🔄 В работе | — |
| `tensa-router` | ⏳ Ждёт | — |
| `tensa-tools` | ⏳ Ждёт | — |
| `tensa-ffi` | ⏳ Ждёт | — |
| `tensa-cli` | ⏳ Ждёт | — |

**Итого тестов:** 166+ (зелёные).

**Приложения:** `apps/desktop`, `apps/android`, `apps/pwa` — ждут.

---

## Что зафиксировано

### Архитектура

- **Rust-ядро**, Cargo workspace, 11 крейтов.
- **Rust + внешние сервисы** (teloxide, himalaya, lettre, caldav).
- **gRPC (tonic)** для синхронизации.
- **uniffi** для FFI-мостов.
- **Lance 13.0** для векторного поиска.
- **fastembed-rs 7.1** для эмбеддингов.
- **Иерархическая память** (Letta-style) на Rust.

### LLM

- **Локальная:** TheStageAI/gemma-4-E2B-it-GGUF (XS/S/M) через llama.cpp FFI.
- **Облачная:** google/gemma-4-26b-a4b-it (free/paid).
- **Fallback:** бесплатная → платная → локальная → degraded.
- **Бюджет:** 100K токенов/день, $10/мес.
- **Инструменты:** tool calls, память, документы, почта, календарь, Telegram, приложения.

### Память

- **Core memory** (SQLite): persona, human, tasks, preferences.
- **Recall memory** (SQLite + FTS5): история разговоров.
- **Archival memory** (Lance): документы, факты.
- **Memory Manager** — координатор.
- **Tool calls:** core_memory_replace, core_memory_append, recall_memory_search, archival_memory_search, archival_memory_insert.
- **Файл памяти** при первом запуске.

### Документы

- **PDF:** pdfplumber-rs.
- **DOCX:** docx-rs.
- **XLSX:** calamine.
- **PPTX:** свой на zip + quick-xml.
- **OCR:** RustO! (MIT) + Cyrillic OCR.
- **Определение сканов:** постранично, 3 правила.

### Синхронизация

- **ApprovalStore:** постоянно, gRPC.
- **Память/заметки/контакты:** on-demand, gRPC.
- **Файлы:** Syncthing (в дистрибутиве).
- **Конфликты:** LWW + векторные часы.

### Безопасность

- **ApprovalStore** — все действия через одобрение.
- **Белый список приложений** — настраиваемый.
- **Чёрный список, запрещённые аргументы.**
- **Аудит** всех действий.
- **Tailscale** — защищённая сеть.

### Платформы

- **MVP:** Windows, Linux, Android.
- **После MVP:** macOS, iOS.
- **PWA:** временный интерфейс для iOS.
- **Точки входа:** Rust компилируется под macOS/iOS в CI.

### UI

- **Десктоп:** Tauri + Svelte.
- **Android:** Flutter + Kotlin.
- **PWA:** Svelte.
- **Плавающий виджет:** две зоны (Т + ...), overlay + App Widget.
- **Spotlight HUD** (десктоп).
- **Approval Center.**
- **Мастер первого запуска.**

### Бизнес

- **Community** (бесплатно).
- **Personal** (499 ₽/мес).
- **Pro** (1 299 ₽/мес).
- **Team** (3 999 ₽/мес).
- **Enterprise** (индивидуально).
- **Точка безубыточности:** год 2.

### Roadmap

- **v0.1.0:** MVP.
- **v0.2.0:** macOS, iOS, WhatsApp, WoL, несколько ящиков, sleep-time compute, синхронизация памяти.
- **v0.3.0:** массовый потребитель.
- **v1.0.0:** пользователи без технических навыков.
- **v2.0.0:** корпоративные клиенты.

---

## Структура репозитория

```
tensa/
├── Cargo.toml
├── Cargo.lock
├── rust-toolchain.toml
├── .cargo/config.toml
├── crates/
│   ├── tensa-core/
│   ├── tensa-approval/
│   ├── tensa-sync/
│   ├── tensa-router/
│   ├── tensa-memory/          # включает core/recall/archival
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
│   ├── ADR-021.md
│   ├── Contract_13.md
│   ├── dop_tz.md
│   ├── dop_tz_core.md
│   ├── FIN.md
│   ├── glm/                   # 8 файлов
│   ├── contracts/             # .proto, UDL, OpenAPI
│   ├── guides/                # 6 руководств
│   └── product/               # 7 частей
└── .github/workflows/
```

---

## Что дальше

### 1. Завершить разработку крейтов

- `tensa-perf` (в работе).
- `tensa-router`.
- `tensa-tools`.
- `tensa-ffi`.
- `tensa-cli`.

### 2. TASK-ENV-001 — окружение для нативного `tensa-llm`

- LLVM/libclang (для bindgen).
- cmake.
- Пинить версию llama.cpp.
- Скачать тестовые GGUF-модели.
- Проверить сборку `llama-sys`.

### 3. Разработка приложений

- `apps/desktop` (Tauri + Svelte).
- `apps/android` (Flutter + Kotlin).
- `apps/pwa` (Svelte).

### 4. Создание репозитория

- GitHub (`github.com/kostyuk230378-cmd/tensa`).
- Cargo workspace.
- CI/CD.
- Первый коммит.

### 5. Продуктовая документация в Notion/Confluence

- Перенос 7 частей.
- Настройка структуры.
- Доступ для команды.

### 6. Участие в конкурсе AutoClaw

- Real Business Track.
- Пост в X с `@AutoClawAIer`.
- Заявка через форму.
- Публикация на GitHub.

---

## Связанные документы

- `00-index.md` — оглавление.
- `01-adr.md` — Architecture Decision Records.
- `02-contracts.md` — Контракты.
- `03-specs.md` — Технические задания.
- `04-examples.md` — Примеры кода.
- `ADR-021.md` — Иерархическая память.
- `Contract_13.md` — Core Memory.
- `dop_tz.md` — Дополнение к ТЗ tensa-memory.
- `dop_tz_core.md` — Дополнение к ТЗ tensa-core.


## Состояние на 2026-10-17

- Код: 14 членов воркспейса (ядро Rust: core/approval/memory/llm/router/tools/server/cli/ffi/nodes/perf/sync,
  обёртки llama.cpp), PWA (Svelte 5), Android (нативный Java-клиент), Windows-установщик (NSIS).
- Тесты: workspace 356 passed (1 ignored); memory Lance+fastembed 35 (1 ignored); llama native 36
  (2 ignored). fmt/clippy чисто; CI зелёный.
- Реализовано сверх документации: `POST /query` (облако/локальная GGUF с fallback), календарь
  (guard конфликтов + ICS), почтовые аккаунты, `/models/embedder` (приём e5-small из APK),
  `tensa memory` на реальной модели, установщики Windows/Android с моделью в комплекте.
- Артефакты: GitHub Releases v0.1.0 — TensaSetup.exe (213 МБ) и Tensa.apk (239 МБ).
