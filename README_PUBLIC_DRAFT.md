<!--
Черновик публичного README для GitHub-репозитория TENSA.
TODO перед публикацией:
  1) CI-бейдж (шаблон закомментирован ниже) — включить после первого прогона workflow на GitHub Actions;
  2) число тестов — снимок прогона `cargo test --workspace` от 2026-10-16 (237); обновить после завершения текущих правок tensa-core.
Остальные числа сверены по файлам репозитория на 2026-10-16.
-->

# Tensa

**Персональный ИИ-секретарь, который работает на ваших устройствах и делает только то, что вы одобрили.**

<!-- TODO: CI-бейдж — раскомментировать после первого зелёного прогона workflow на GitHub Actions:
     ![CI](https://github.com/kostyuk230378-cmd/TENSA/actions/workflows/ci.yml/badge.svg)
CI-статус: TODO. -->

---

## Что это

Tensa — персональный ИИ-секретарь для одного человека. Он помогает разбираться
с потоком информации: документы, почта, календарь, заметки, контакты. Найти
смету за апрель, подготовить письмо, поставить встречу в календарь — команда
формулируется текстом или голосом, а рутину секретарь берёт на себя. Это не
«чат-бот общего назначения», а специализированный секретарь, который знает
именно ваши документы, ваши контакты и ваш календарь.

Tensa живёт на нескольких узлах (десктоп, Android) и синхронизирует их между
собой по gRPC. Ядро написано на Rust, вывод модели выполняет локальный
инференс (llama.cpp, GGUF), а облачная модель подключается опционально. Всё,
что меняет мир за пределами приложения — отправить письмо, создать событие,
запустить программу — проходит через ApprovalStore: сначала одобрение
владельца, потом выполнение.

Проект в активной разработке. Готовы восемь крейтов ядра, `cargo test
--workspace` проходит полностью (237 тестов зелёные на 2026-10-16), а в
репозитории лежит вся документация: ADR, контракты, технические задания,
продуктовая документация и пользовательские руководства.

---

## Ключевые принципы

**Privacy-first.** Данные и модели — на ваших устройствах. Локальное
хранилище (SQLite + FTS5, векторный поиск), локальный инференс, связь между
узлами по защищённой сети (Tailscale). Облако — не обязательная часть
продукта, а осознанно подключаемая опция.

**Действия — только через ApprovalStore.** LLM не выполняет сетевых действий
напрямую: сначала действие попадает в очередь одобрения и живёт свой
жизненный цикл (`created → pending → approved → executed`), с отказами,
истечением по TTL (24 часа) и ошибками. Каждое изменение состояния — событие
для синхронизации между узлами (event sourcing).

**Локальная LLM по умолчанию, облако — опционально.** Базовая модель —
Gemma 4 E2B (GGUF через llama.cpp). Если локальный инференс недоступен,
маршрутизатор уходит в облако по цепочке fallback (бесплатная модель →
платная → локальная → degraded) и ведёт бюджет (100 000 токенов/день,
$10/мес).

---

## Архитектура

```
┌─────────────────────────────────────────────────────────────────────┐
│  Приложения (в работе)                                              │
│  desktop: Tauri + Svelte · android: Flutter + Kotlin · pwa: Svelte  │
└───────────────────────────────┬─────────────────────────────────────┘
                                │ tensa-ffi (uniffi) · PWA HTTP API
                                ▼
┌─────────────────────────────────────────────────────────────────────┐
│  tensa-core — общие типы, конфигурация, заметки/контакты,           │
│  координация подсистем, мастер первого запуска                      │
└───────────────────────────────┬─────────────────────────────────────┘
                                │
   ┌───────────────┬────────────┴─────────┬────────────┬──────────────┐
   ▼               ▼                      ▼            ▼              ▼
 tensa-approval  tensa-memory        tensa-router  tensa-nodes   tensa-perf
 одобрение       core / recall /     маршрут LLM:  реестр        профиль
 действий:       archival;           локальная →   узлов,        устройства:
 очередь,        SQLite + FTS5       облачная      heartbeat,    CPU, память,
 события,        (Lance и fastembed  (опционально) роли          батарея,
 векторные       — за фичами)                      heavy/thin    термалка
 часы             │                      │
   │              │                      ▼
   │              │             tensa-llm — llama.cpp v0.7.0,
   │              │             GGUF Gemma 4 E2B (локально)
   └──────────────┴──────────► tensa-sync — gRPC (tonic) между узлами;
                               файлы между узлами — Syncthing
```

### Крейты

| Крейт | Назначение | Статус |
|---|---|---|
| `tensa-approval` | ApprovalStore: очередь одобрения действий, события, векторные часы (Контракт 1) | готов |
| `tensa-core` | Общие типы, конфигурация, заметки/контакты (SQLite + FTS5), координация подсистем, мастер первого запуска (Контракты 3/4/13) | готов |
| `tensa-memory` | Иерархическая память core/recall/archival, чанкинг, кэш; векторный поиск Lance и эмбеддинги fastembed — за фичами (Контракты 2/13) | готов |
| `tensa-nodes` | Реестр узлов: heartbeat, роли heavy/thin/auto, ресурсы, управление соединениями (Контракт 7) | готов |
| `tensa-sync` | Синхронизация узлов: gRPC (tonic), журнал, heartbeat, соединения, Syncthing (Контракт 8) | готов |
| `tensa-llm` (+ `tensa-llama`, `tensa-llama-sys`) | Локальный инференс: двухуровневый FFI к llama.cpp, GGUF, контроль версий и автообновление с подтверждением | готов (без нативного слоя, см. ниже) |
| `tensa-perf` | Профилирование устройства (CPU, память, батарея, термалка) и динамические роли heavy/thin; детекция GPU/NPU — заготовки до TASK-ENV-001 | готов |
| `tensa-router` | Маршрутизация к локальной/облачной LLM и выбор узла инференса, бюджет, fallback | готов |
| `tensa-tools` | Инструменты: документы, почта, календарь, Telegram, запуск приложений | в работе |
| `tensa-ffi` | FFI-мосты (uniffi) для приложений | в работе |
| `tensa-cli` | Интерфейс командной строки | в работе |
| apps: `desktop`, `android`, `pwa` | Приложения: Tauri + Svelte, Flutter + Kotlin, Svelte | в работе |

Строки со статусом «в работе» — запланированные крейты и приложения: в
workspace их пока нет, состав и порядок заданы в `teh_doc/FIN.md` и ТЗ.

Проверка на 2026-10-16 (rustc 1.98.1): `cargo test --workspace` — 237 тестов,
0 упавших. Распределение: `tensa-approval` 15, `tensa-core` 37,
`tensa-memory` 31, `tensa-nodes` 24, `tensa-sync` 30, `tensa-llm` вместе с
подкрейтами 30, `tensa-perf` 31, `tensa-router` 39.

---

## Быстрый старт

Нужен Rust (edition 2021; в `Cargo.toml` заявлен `rust-version = "1.75"`).
Сборка и тесты — без внешних зависимостей:

```bash
git clone https://github.com/kostyuk230378-cmd/TENSA
cd TENSA

cargo build --workspace
cargo test --workspace
```

Опциональные фичи памяти (`lance`, `fastembed`) требуют `protoc`:

```bash
cargo test -p tensa-memory --features lance,fastembed
```

### Нативный инференс (фича `native`)

По умолчанию `tensa-llama-sys` компилируется пустым слоем: `NATIVE_READY ==
false`, а `tensa-llama` честно возвращает `EngineUnavailable`. Настоящий
инференс llama.cpp включается фичей `native` (она протянута по цепочке
`tensa-llm` → `tensa-llama` → `tensa-llama-sys`) и требует cmake, libclang и
компилятор C++.

```powershell
# 1) исходники закреплённой версии llama.cpp v0.7.0 (в git не попадают, каталог vendor/ в .gitignore)
git clone --depth 1 --branch v0.7.0 https://github.com/ggml-org/llama.cpp vendor/llama.cpp

# 2) libclang (вариант без прав администратора — pip-пакет в пользовательский Python)
python -m pip install libclang
$env:LIBCLANG_PATH = "<site-packages>\clang\native"   # каталог с libclang.dll

# 3) сборка нативного слоя
cargo build -p tensa-llama-sys --features native
```

Модель для тестов — GGUF-файл (~1.6 ГБ) в `~/.tensa/models/` (имя
`gemma-4-E2B-it-XS-TS-Q3_K_S.gguf`) либо путь в переменной `TENSA_TEST_MODEL`:

```bash
cargo test -p tensa-llama --features native -- --ignored
```

Тесты с реальной моделью помечены `#[ignore]` (инференс долгий) и запускаются
флагом `--ignored`. Подробности — в
[`crates/tensa-llm/llama-sys/README.md`](crates/tensa-llm/llama-sys/README.md):
там же про `LLAMA_CPP_DIR`, кэш сборки в `target/` и про то, что GPU/OpenCL
бэкенды выключены (CPU-инференс, SIMD через `GGML_NATIVE`).

---

## Структура репозитория

| Путь | Содержимое |
|---|---|
| `crates/` | Крейты: восемь готовых ядра + подкрейты LLM-слоя `tensa-llm/llama` и `tensa-llm/llama-sys` |
| `teh_doc/` | Техническая документация: ADR (21, включая `ADR-021.md`), контракты (13, включая `Contract_13.md`), ТЗ (14), примеры, `FIN.md`; машинные контракты — в `teh_doc/contracts/` (6 `.proto`, `tensa.udl`, `openapi.yaml`) |
| `FROM_GLM/` | Внутреннее руководство для AI-разработчика (формат задач, тестирование, ревью, CI) и системный промпт Tensa |
| `PRODUCT DOC/` | Продуктовая документация: `PD_1`…`PD_7` — продукт, аудитория, бизнес-модель, roadmap, память и персона, маркетинг, пользовательские руководства |
| `Guide/` | Пользовательские руководства: Tailscale, iOS/PWA, WhatsApp Cloud API, sysadmin, WoL, always-on |
| `.github/workflows/ci.yml` | CI: `cargo fmt --all --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; отдельный job для памяти с `lance`/`fastembed` |
| `apps/` | Приложения (desktop, android, pwa) — в плане, в репозитории пока нет |

В корне также: `Cargo.toml` / `Cargo.lock`, `START_HERE.md` (карта
документации), `AUDIT_READY_CRATES.md` (аудит готовых крейтов), `LICENSE-MIT`,
`LICENSE-APACHE`.

---

## Статус проекта

**Готово**

- Восемь крейтов ядра и подкрейты LLM-слоя; `cargo test --workspace` — 237
  тестов, 0 упавших (2026-10-16).
- Конфигурация CI: форматирование, clippy без предупреждений, тесты, отдельный
  job для векторных фич памяти. Статус прогонов на GitHub Actions — TODO
  (бейдж не добавлен).
- Документация: 21 ADR, 13 контрактов, 14 ТЗ, продуктовая документация (7
  частей), 6 пользовательских руководств.
- Аудит готовых крейтов с разбором отклонений от ТЗ — `AUDIT_READY_CRATES.md`.

**Что дальше** (кратко по `teh_doc/FIN.md`, раздел «Что дальше»)

1. TASK-ENV-001 — окружение для нативного `tensa-llm`: cmake, LLVM/libclang,
   пиннинг версии llama.cpp, тестовые GGUF-модели, проверка сборки `llama-sys`;
   далее `litert/` (NPU) и NVML (GPU в `tensa-perf`).
2. Реальные `LanceMemoryStore` и fastembed-эмбеддер за фичами `lance`/
   `fastembed` — API-поверхность уже зафиксирована
   `crates/tensa-memory/tests/api_surface.rs`.
3. Крейты `tensa-tools`, `tensa-ffi`, `tensa-cli`.
4. Приложения: `apps/desktop` (Tauri + Svelte), `apps/android` (Flutter +
   Kotlin), `apps/pwa` (Svelte).
5. Перенос продуктовой документации в Notion/Confluence, публикация
   репозитория, заявка на конкурс.

---

## Лицензия

Распространяется под лицензией **MIT OR Apache-2.0** — на выбор; тексты
лицензий в файлах [LICENSE-MIT](LICENSE-MIT) и [LICENSE-APACHE](LICENSE-APACHE).
Если вы явно не указали иное, любой вклад, отправленный в этот проект,
лицензируется на тех же условиях (dual licensing, как принято в экосистеме
Rust).

Репозиторий участвует в конкурсе **AutoClaw** — Real Business Track (дедлайн
22.10.2026).

---

## In English

Tensa is a privacy-first personal AI secretary written in Rust. Inference runs locally (llama.cpp v0.7.0, Gemma 4 E2B GGUF), cloud models are optional, and nodes sync with each other over gRPC.
Every action the assistant wants to take goes through the ApprovalStore queue and needs the owner's approval before execution.
Build and test in two commands: `cargo build --workspace` and `cargo test --workspace` (237 tests green as of 2026-10-16); native inference lives behind the `native` feature (see `crates/tensa-llm/llama-sys/README.md`).
Eight core crates are implemented; `tensa-tools`, `tensa-ffi`, `tensa-cli` and the desktop / Android / PWA apps are next. Documentation (ADRs, contracts, specs, product docs, guides) is in `teh_doc/`, `PRODUCT DOC/` and `Guide/`.
Licensed under MIT OR Apache-2.0. Submitted to the AutoClaw competition (Real Business Track).
