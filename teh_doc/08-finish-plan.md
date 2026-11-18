# Руководство для GLM 5.3 — План завершения проекта

**Версия:** 0.1.0
**Дата:** 2026-11-09
**Статус:** План завершения
**Связано с:** `00-overview.md` … `07-ci.md`, `system_prompt_tensa.md`, `dop_tz.md`, `dop_tz_core.md`

---

## Назначение

Этот документ описывает, что осталось сделать, чтобы Tensa
соответствовал собственной документации: системному промпту
секретаря, доп. ТЗ по памяти и ядру, quality gates из
`02-testing.md` и `07-ci.md`.

Документ — рабочий план для GLM 5.3. Он не заменяет `01-task-format.md`:
каждая задача внутри фазы оформляется по тому формату, с отчётом
Архитектору и ревью.

**Конкурс AutoClaw — вне scope этого документа.** Не трогай X,
формы, скриншоты подписки. Только код, тесты, документация, CI.

---

## Исходное состояние (09.11.2026)

Проверено read-only, точка `main` / `3c9426c`.

| Контур | Факт |
|---|---|
| Код и сборка | 17 крейтов, ~31 404 строк Rust, рабочее дерево чистое |
| CI | run #91 зелёный (Ubuntu, 3m29s) |
| Релиз | v0.2.28, APK и Setup побайтово совпадают с локальными |
| Тесты | 388 passed, 1 ignored (`--exclude tensa-whisper`) |
| fmt / clippy | проходят (clippy — 0 warnings) |
| Windows | `cargo test --workspace` падает на `whisper-rs-sys` (нет libclang) |
| Production-код | 0 unwrap/panic/todo, 212/212 файлов с doc-комментариями |
| Заглушки | first-run, contacts, apps — 501; sync/nodes — 501/stub |
| Частично | email (SMTP без IMAP), calendar (JSON без CalDAV) |
| Tool-calls | только облачный fallback, локальная GGUF — нет |
| Документация | README устарел, BUILD_REPORT — про v0.2.24 |

**Главная проблема:** системный промпт секретаря и доп. ТЗ обещают
больше, чем реализовано. Quality gates из `02`/`07` не подтверждены.

---

## Принципы работы над планом

### 1. Одна фаза — одно ревью

Не начинай следующую фазу, пока предыдущая не принята Архитектором.

### 2. Одна задача — один отчёт

Внутри фазы — по одной задаче за раз, по формату
`01-task-format.md`. Большие коммиты не принимаются.

### 3. Контракты не меняются

Сигнатуры из `system_prompt_tensa.md`, `dop_tz.md`, `dop_tz_core.md`,
контрактов 1–13 — обязательны. Если неудобно — предложи Архитектору,
не меняй молча.

### 4. Тесты вместе с кодом

Без тестов задача не принимается (`02-testing.md`).

### 5. Документация вместе с кодом

Изменил API — обнови `///`, README крейта, пример
(`04-documentation.md`).

### 6. Никаких секретов

Не логируй payload, содержимое писем, токены. Не коммить `.env`,
`secrets.*`.

---

## Фазы

### Фаза 0 — Гигиена workspace

**Цель:** убрать расхождения, которые мешают всему остальному.

**Задачи:**

1. `git fetch --tags` — привести локальные теги к релизам v0.2.26–v0.2.28.
   `git describe` не должен давать `v0.2.2-44-g3c9426c`.
2. `README.md`:
   - 17 крейтов (не 14);
   - актуальное число тестов (388 passed, 1 ignored);
   - раздел **«Windows: LLVM/libclang для сборки»** — `LIBCLANG_PATH`,
     установка LLVM, почему падает `whisper-rs-sys`;
   - единицы MiB/MB привести к одному виду.
3. `BUILD_REPORT.md` — пометить как исторический (v0.2.24), не
   переписывать под v0.2.28.
4. `workspace.package.version` — **согласовать с Архитектором**:
   поднять до `0.2.28` или задокументировать, почему `0.1.0`.
   Без подтверждения не менять.
5. Убрать или заменить старый APK в `target/release/pwa/static`
   (хеш от 17.10 не совпадает с релизным).
6. Проверить, что каждая SQL-миграция имеет down-миграцию
   (`05-rollback.md`). Если нет — дописать.

**Критерии:** `git describe` корректен; README соответствует факту;
`cargo test --workspace --exclude tensa-whisper --no-fail-fast`
проходит на чистой машине после установки libclang по README.

### Фаза 1 — First-run wizard

**Цель:** закрыть `first-run — 501`, реализовать `dop_tz_core.md`.

**Задачи:**

1. `FirstRunWizard` с 8 шагами: language, introduction, models,
   telegram, email, whatsapp, tailscale, finish.
2. `TensaCore::is_first_run`, `run_first_run_wizard`, `init_memory`.
3. `DefaultMemoryManager::init_default(InitParams)` — четыре блока
   core memory (persona, human, tasks, preferences) с текстами из ТЗ.
4. Tauri IPC команды `wizard_step_*` — по списку из `dop_tz_core.md`.
5. uniffi UDL `FirstRunWizard` — по списку из ТЗ.
6. PWA HTTP API `/first-run/*` — по списку из ТЗ.

**Тесты:** unit на каждый шаг; integration на полный цикл;
error-тесты на неверный Telegram-токен, неверный IMAP, ошибку
загрузки моделей, ошибку сохранения конфига.

**Ссылки:** `dop_tz_core.md`, Контракты 4, 9, 10, 11, 12, 13, ADR-021.

### Фаза 2 — Инструменты секретаря

**Цель:** закрыть 501 и частичные реализации из
`system_prompt_tensa.md`. Сигнатуры — строго из документа.

**Задачи:**

1. **Contacts** (`contacts — 501`):
   - `create_contact(name, email, phone, telegram, whatsapp)`;
   - `find_contact(query)`.
2. **Apps** (`apps — 501`):
   - `launch_app(name)`;
   - `open_url(url)`.
3. **Email IMAP** (сейчас только SMTP):
   - `fetch_emails(account_id, folder, limit)`;
   - `search_emails(account_id, query)`.
   SMTP-отправку сохранить.
4. **Calendar CalDAV** (сейчас JSON без CalDAV):
   - `create_calendar_event(...)`;
   - `check_calendar(start, end)`;
   - `list_calendar_events(start, end)`.
   JSON-формат сохранить, если используется.

**Тесты:** unit на каждый метод; integration на реальный
контакт/письмо/событие (in-memory или локальный mock-сервер);
error-тесты на невалидный email, отсутствующий контакт, конфликт
календаря.

**Ссылки:** `system_prompt_tensa.md`, Контракт 1, ADR-012.

### Фаза 3 — Tool-calls памяти и локальной модели

**Цель:** закрыть `dop_tz.md` и протокол `system_prompt_tensa.md`.

**Задачи:**

1. **Memory tool calls** из `dop_tz.md`:
   - `core_memory_replace`, `core_memory_append`;
   - `recall_memory_search`, `archival_memory_search`,
     `archival_memory_insert`.
2. Проверить `MemoryManager::get_context`: возвращает
   `MemoryContext { core_blocks, recent_messages, total_tokens }` и
   укладывается в `max_context_tokens`.
3. **Tool-calls локальной модели.** Сейчас только облачный fallback.
   Нужно, чтобы локальная GGUF-модель возвращала JSON tool-calls по
   протоколу из `system_prompt_tensa.md` (одно действие / steps /
   ask_user / error). Если модель не обучена — парсер ответа +
   fallback на `ask_user`.

**Тесты:** unit на каждый tool call; integration на полный цикл
«LLM вернул JSON → MemoryManager применил → get_context содержит
изменение»; error-тесты на невалидный JSON, неизвестный инструмент,
превышение `max_block_tokens`.

**Ссылки:** `dop_tz.md`, `system_prompt_tensa.md`, Контракт 2,
Контракт 13, ADR-021.

### Фаза 4 — Sync/nodes

**Цель:** заменить `sync/nodes — HTTP 501 / CLI stub` на реализацию
по ADR-012, ADR-013, Контракт 8.

**Задачи:**

1. **Сначала** — предложение Архитектору: схема gRPC, модель
   `NodeRegistry`, стратегия on-demand sync, обработка конфликтов.
   Код — после утверждения.
2. gRPC (tonic) по Контракту 8.
3. `NodeRegistry`: регистрация, heartbeat, список узлов.
4. On-demand sync по ADR-013.
5. Заменить stub на реальную реализацию.

**Тесты:** unit на merge vector clock; integration на два узла
(in-memory), конфликт версий, идемпотентность; error-тесты на
недоступный узел, несовпадение версий.

**Ссылки:** ADR-012, ADR-013, Контракт 8.

### Фаза 5 — Quality gates

**Цель:** подтвердить `02-testing.md` и `07-ci.md` фактом.

**Задачи:**

1. **Покрытие:** `cargo llvm-cov --workspace --all-features
   --fail-under-lines 60`; для критичных крейтов (approval, memory,
   core) — ≥80%. Цифры — в отчёт.
2. **Doc-tests:** `cargo test --workspace --doc` проходит.
3. **E2E:** Playwright для десктопа — сценарий «approve action from
   UI» из `02-testing.md`.
4. **Property-тесты:** `proptest` на chunker и vector clock.
5. **Benchmark:** `criterion` на `search_10k` из `02-testing.md`.
6. **CI-матрица по `07-ci.md`:** fmt, clippy, test на
   ubuntu/windows/macos, coverage, audit, android, ios, macos.
   Сейчас CI #91 — только Ubuntu.
7. **Release-workflow:** `.github/workflows/release.yml` по шаблону
   из `07-ci.md`. Сейчас APK/Setup собираются и грузятся вручную.
8. **Pre-commit hook** из `07-ci.md` — добавить в репозиторий.

**Критерии:** CI зелёный по всем jobs; покрытие ≥ порога; release
собирается workflow, а не руками; pre-commit установлен.

**Ссылки:** `02-testing.md`, `07-ci.md`.

### Фаза 6 — Документация

**Цель:** привести документацию к `04-documentation.md`.

**Задачи:**

1. Публичный API — `///` на английском (сейчас doc-комментарии
   русские; выборочно проверить, публичный API привести к стандарту).
2. README каждого крейта — по шаблону из `04-documentation.md`
   (назначение, принципы, использование, структура, тесты, ADR,
   контракты).
3. `examples/` — по одному примеру на публичный API, компилируется.
4. `cargo doc --no-deps` и `cargo test --doc` проходят.

**Ссылки:** `04-documentation.md`.

---

## Критерии приёмки (для каждой задачи)

- `cargo check --workspace --all-targets` — OK.
- `cargo test -p <крейт> --all-features` — OK.
- `cargo fmt --all -- --check` — без изменений.
- `cargo clippy --workspace --exclude tensa-whisper --all-targets
  --all-features -- -D warnings` — 0 warnings.
- `cargo test -p <крейт> --doc` — OK.
- Покрытие не упало ниже текущего уровня.
- Документация обновлена (README крейта, `///` на публичном API).
- Отчёт Архитектору по формату `01-task-format.md`.

---

## Тесты (обязательный минимум)

1. **Unit** — каждая новая функция/метод.
2. **Integration** — взаимодействие с БД/сетью/mock-сервером.
3. **Error** — невалидные входные данные, отсутствующие ресурсы,
   конфликты.
4. **Property** — для алгоритмов (chunker, vector clock, merge).
5. **Doc-test** — для каждого публичного API с примером.

Именование: `test_<что>_<условие>_<ожидание>` (`02-testing.md`).

---

## Порядок работы

1. Прочитай `00-overview.md` … `07-ci.md` полностью.
2. Прочитай `system_prompt_tensa.md`, `dop_tz.md`, `dop_tz_core.md`.
3. Начни с **Фазы 0**. Отчитайся Архитектору.
4. Дождись ревью. Переходи к **Фазе 1**.
5. Внутри каждой фазы — одна задача = один отчёт = одно ревью.
6. Не начинай следующую фазу, пока предыдущая не принята.

**Если неясно — ask_user. Не додумывай. Не меняй контракты.**

---

## Что НЕ делать

- Не менять сигнатуры из контрактов и `system_prompt_tensa.md`.
- Не добавлять зависимости без утверждения Архитектора.
- Не использовать `unwrap()` в production-коде (только `Result`).
- Не логировать секреты, payload, содержимое писем.
- Не обходить ApprovalStore.
- Не трогать конкурс AutoClaw, X-посты, формы, скриншоты.
- Не переписывать `BUILD_REPORT.md` под v0.2.28 — только пометить.
- Не менять `workspace.package.version` без подтверждения.
- Не начинать следующую фазу без ревью предыдущей.
- Не делать большие коммиты — по одной задаче за раз.

---

## Чек-лист перед стартом новой сессии

- [ ] Прочитал `00-overview.md` … `07-ci.md`.
- [ ] Прочитал `system_prompt_tensa.md`, `dop_tz.md`, `dop_tz_core.md`.
- [ ] Прочитал этот файл полностью.
- [ ] Знаю, какая фаза текущая.
- [ ] Знаю, какая задача внутри фазы текущая.
- [ ] Готов оформить задачу по `01-task-format.md`.
- [ ] Готов отчитаться Архитектору по формату из `01-task-format.md`.
- [ ] Не трогаю конкурс AutoClaw.

---

## Частые ошибки

### 1. Начать с Фазы 1, пропустив Фазу 0

**Плохо:** сразу first-run wizard, README устарел, libclang не описан.

**Хорошо:** Фаза 0 → ревью → Фаза 1.

### 2. Менять сигнатуры «для удобства»

**Плохо:** «сигнатура из `system_prompt_tensa.md` неудобна — изменил».

**Хорошо:** «предлагаю изменить сигнатуру, потому что [факт]».

### 3. Закрыть 501 заглушкой

**Плохо:** вернуть `Ok(())` без реальной реализации.

**Хорошо:** реальная реализация + тесты.

### 4. Пропустить тесты «потом»

**Плохо:** код без тестов, «тесты в следующей задаче».

**Хорошо:** код + тесты вместе, как в `02-testing.md`.

### 5. Забыть про локальную модель

**Плохо:** tool-calls только в облачном fallback, локальная GGUF
игнорируется.

**Хорошо:** парсер JSON tool-calls + fallback на `ask_user`.

### 6. Забыть про CI-матрицу

**Плохо:** CI только Ubuntu, release руками.

**Хорошо:** матрица по `07-ci.md`, release-workflow.

### 7. Трогать конкурс

**Плохо:** X-посты, формы, скриншоты подписки.

**Хорошо:** только код, тесты, документация, CI.

### 8. Большой коммит

**Плохо:** 1000 строк в одном коммите.

**Хорошо:** по одной задаче за раз, как в `03-review.md`.

---

## Связанные файлы

- `C:\Mi Work\my-project\PROJECT\TENSORLOGIX\TENSA\FROM_GLM\00-overview.md` — обзор.
- `01-task-format.md` — формат задач.
- `02-testing.md` — тестирование.
- `03-review.md` — ревью.
- `04-documentation.md` — документирование.
- `05-rollback.md` — откат.
- `06-cargo-workspace.md` — работа с workspace.
- `07- C:\Mi Work\my-project\PROJECT\TENSORLOGIX\TENSA\FROM_GLM\07-ci.md.
- `system_prompt_tensa.md` — контракт инструментов секретаря.
- `dop_tz.md` — доп. ТЗ tensa-memory.
- `dop_tz_core.md` — доп. ТЗ tensa-core.