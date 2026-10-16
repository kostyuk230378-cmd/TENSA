Принято. `05-rollback.md` утверждён. Переходим к `06-cargo-workspace.md`.

---

## Документация для GLM 5.3: `06-cargo-workspace.md`

```markdown
# Руководство для GLM 5.3 — Работа с Cargo workspace

**Версия:** 0.1.0
**Дата:** 2026-10-06
**Статус:** Cargo workspace

---

## Назначение

Этот документ описывает, как GLM 5.3 должен работать с Cargo
workspace Tensa. Цель: единая структура, единые версии, быстрая
сборка, чистая архитектура.

---

## Принципы

### 1. Один workspace — один проект

Все крейты Tensa — в одном workspace. Нет отдельных репозиториев.

### 2. Единые версии зависимостей

Все крейты используют одни и те же версии зависимостей через
`[workspace.dependencies]`.

### 3. Единый lock-файл

Один `Cargo.lock` на весь workspace.

### 4. Изоляция крейтов

Каждый крейт — отдельный crate. Зависимости — явные.

### 5. Сборка по частям

Можно собирать отдельные крейты (`cargo build -p tensa-approval`).

---

## Структура workspace

```
tensa/
├── Cargo.toml                    # workspace
├── Cargo.lock                    # единый lock
├── rust-toolchain.toml           # версия Rust
├── .cargo/
│   └── config.toml               # настройки Cargo
├── crates/
│   ├── tensa-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   ├── tensa-approval/
│   │   ├── Cargo.toml
│   │   ├── schema.sql
│   │   └── src/
│   ├── tensa-sync/
│   ├── tensa-router/
│   ├── tensa-memory/
│   ├── tensa-nodes/
│   ├── tensa-tools/
│   │   ├── Cargo.toml
│   │   ├── telegram/
│   │   ├── email/
│   │   ├── calendar/
│   │   ├── apps/
│   │   ├── documents/
│   │   ├── ocr/
│   │   ├── whatsapp/
│   │   └── wol/
│   ├── tensa-llm/
│   │   ├── Cargo.toml
│   │   ├── llama-sys/
│   │   └── llama/
│   ├── tensa-perf/
│   ├── tensa-ffi/
│   └── tensa-cli/
├── apps/
│   ├── desktop/
│   │   ├── src-tauri/
│   │   │   ├── Cargo.toml
│   │   │   └── src/
│   │   └── src/
│   ├── android/
│   └── pwa/
├── docs/
└── .github/
    └── workflows/
```

---

## Корневой `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/tensa-core",
    "crates/tensa-approval",
    "crates/tensa-sync",
    "crates/tensa-router",
    "crates/tensa-memory",
    "crates/tensa-nodes",
    "crates/tensa-tools",
    "crates/tensa-tools/telegram",
    "crates/tensa-tools/email",
    "crates/tensa-tools/calendar",
    "crates/tensa-tools/apps",
    "crates/tensa-tools/documents",
    "crates/tensa-tools/ocr",
    "crates/tensa-tools/whatsapp",
    "crates/tensa-tools/wol",
    "crates/tensa-llm",
    "crates/tensa-llm/llama-sys",
    "crates/tensa-llm/llama",
    "crates/tensa-perf",
    "crates/tensa-ffi",
    "crates/tensa-cli",
    "apps/desktop/src-tauri",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.75"
authors = ["Tensa Contributors"]
license = "MIT OR Apache-2.0"
repository = "https://github.com/tensorlogix/tensa"
homepage = "https://tensa.tensorlogix.com"
documentation = "https://docs.tensa.tensorlogix.com"

[workspace.dependencies]
# Асинхронность
tokio = { version = "1.35", features = ["full"] }
async-trait = "0.1"
futures = "0.3"

# Сериализация
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
toml = "0.8"

# БД
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite", "chrono", "uuid"] }

# Векторный поиск
# Обновлено 2026-10-15 (апрув Архитектора): lance 0.9/fastembed 4.0
# не компилируются rustc 1.98 (arrow-arith 50, quarter() vs chrono).
# API проверен: tensa-memory/tests/api_surface.rs.
lance = "13.0"
fastembed = "7.1"

# LLM
# llama-cpp-sys = { git = "https://github.com/ggml-org/llama.cpp", tag = "b4000" }

# gRPC
tonic = "0.11"
prost = "0.12"
tonic-build = "0.11"

# FFI
uniffi = "0.25"
uniffi-bindgen = "0.25"

# HTTP
reqwest = { version = "0.11", features = ["json", "rustls-tls"] }
axum = "0.7"

# Telegram
teloxide = "0.12"

# Email
himalaya = "0.9"
lettre = "0.11"

# CalDAV
caldav = "0.4"

# PDF
pdfplumber-rs = "0.1"

# DOCX
docx-rs = "0.4"

# XLSX
calamine = "0.24"

# OCR
rusto-rs = "0.1"

# Утилиты
uuid = { version = "1.6", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
thiserror = "1.0"
anyhow = "1.0"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }
sha2 = "0.10"
lru = "0.12"
sysinfo = "0.30"
directories = "5.0"
notify = "6.1"
semver = "1.0"

# CLI
clap = { version = "4.4", features = ["derive"] }
colored = "2.1"
indicatif = "0.17"
comfy-table = "7.1"

# Тесты
proptest = "1.4"
mockall = "0.12"
criterion = { version = "0.5", features = ["async_tokio"] }

# Логирование
log = "0.4"

# Профили
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"

[profile.dev]
opt-level = 0
debug = true

[profile.test]
opt-level = 1

[profile.bench]
opt-level = 3
lto = "thin"
```

---

## `Cargo.toml` крейта

```toml
[package]
name = "tensa-approval"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
authors.workspace = true
license.workspace = true
repository.workspace = true

[dependencies]
# Внутренние
tensa-core = { path = "../tensa-core" }

# Внешние (из workspace)
tokio.workspace = true
async-trait.workspace = true
serde.workspace = true
serde_json.workspace = true
sqlx.workspace = true
uuid.workspace = true
chrono.workspace = true
thiserror.workspace = true
tracing.workspace = true

[dev-dependencies]
proptest.workspace = true
mockall.workspace = true

[features]
default = []
test-utils = []
```

**Правила:**
- `version.workspace = true` — версия из workspace.
- `edition.workspace = true` — edition из workspace.
- Внутренние зависимости — `path = "../tensa-core"`.
- Внешние — `.workspace = true`.
- Dev-зависимости — в `[dev-dependencies]`.
- Фичи — в `[features]`.

---

## Команды Cargo

### Сборка

```bash
# Весь workspace
cargo build

# Один крейт
cargo build -p tensa-approval

# Release
cargo build --release

# Под конкретную платформу
cargo build --target aarch64-linux-android

# С фичами
cargo build -p tensa-approval --features test-utils
```

### Тесты

```bash
# Весь workspace
cargo test

# Один крейт
cargo test -p tensa-approval

# Один тест
cargo test -p tensa-approval test_request_action

# С выводом
cargo test -- --nocapture

# Doc-tests
cargo test --doc
```

### Линтинг

```bash
# Форматирование
cargo fmt --all -- --check

# Clippy
cargo clippy --all-targets --all-features -- -D warnings
```

### Документация

```bash
# Сборка документации
cargo doc --no-deps --open

# Все крейты
cargo doc --workspace --no-deps
```

### Проверка

```bash
# Проверка компиляции без сборки
cargo check --workspace

# Проверка всех целей
cargo check --workspace --all-targets

# Проверка всех фич
cargo check --workspace --all-features
```

### Обновление

```bash
# Обновить все зависимости
cargo update

# Обновить конкретную
cargo update -p tokio

# Зафиксировать версию
cargo update -p tokio --precise 1.35.0
```

### Дерево зависимостей

```bash
# Дерево
cargo tree

# Дерево одного крейта
cargo tree -p tensa-approval

# Дубликаты
cargo tree --duplicates
```

### Аудит

```bash
# Уязвимости
cargo audit

# Устаревшие
cargo outdated
```

---

## Порядок зависимостей

### Правило: без циклов

```
tensa-core
    ↑
    ├── tensa-approval
    ├── tensa-memory
    ├── tensa-nodes
    ├── tensa-router
    ├── tensa-sync
    └── tensa-tools
            ↑
        tensa-ffi
            ↑
        tensa-cli
```

**Правила:**
- `tensa-core` — фундамент, ни от кого не зависит.
- Остальные крейты зависят от `tensa-core`.
- `tensa-ffi` зависит от всех.
- `tensa-cli` зависит от всех.
- Циклов быть не должно.

### Проверка циклов

```bash
cargo tree -p tensa-approval
```

Если видишь цикл — ошибка архитектуры.

---

## Фичи

### Правила

- `default` — фичи по умолчанию.
- `test-utils` — утилиты для тестов.
- `full` — все фичи.

### Пример

```toml
[features]
default = []
test-utils = []
full = ["test-utils"]
```

### Использование

```bash
cargo build -p tensa-approval --features test-utils
cargo build -p tensa-approval --all-features
```

---

## Профили

### `release`

```toml
[profile.release]
opt-level = 3
lto = "thin"
codegen-units = 1
strip = true
panic = "abort"
```

**Что даёт:**
- `opt-level = 3` — максимальная оптимизация.
- `lto = "thin"` — link-time optimization.
- `codegen-units = 1` — лучше оптимизация, но дольше сборка.
- `strip = true` — меньше бинарник.
- `panic = "abort"` — меньше бинарник, но нельзя ловить панику.

### `dev`

```toml
[profile.dev]
opt-level = 0
debug = true
```

**Что даёт:**
- Быстрая сборка.
- Debug-информация.

### `test`

```toml
[profile.test]
opt-level = 1
```

**Что даёт:**
- Тесты быстрее.

### `bench`

```toml
[profile.bench]
opt-level = 3
lto = "thin"
```

**Что даёт:**
- Бенчмарки на оптимизированном коде.

---

## Кросс-компиляция

### Android

```bash
# Установить target
rustup target add aarch64-linux-android
rustup target add armv7-linux-androideabi

# Установить NDK
# ...

# Собрать
cargo build --release --target aarch64-linux-android
```

### iOS

```bash
# Установить target
rustup target add aarch64-apple-ios
rustup target add x86_64-apple-ios-sim

# Собрать
cargo build --release --target aarch64-apple-ios
```

### macOS

```bash
# Установить target
rustup target add x86_64-apple-darwin
rustup target add aarch64-apple-darwin

# Собрать
cargo build --release --target aarch64-apple-darwin
```

---

## `.cargo/config.toml`

```toml
[target.aarch64-linux-android]
linker = "aarch64-linux-android21-clang"
ar = "aarch64-linux-android-ar"

[target.armv7-linux-androideabi]
linker = "armv7a-linux-androideabi21-clang"
ar = "armv7a-linux-androideabi-ar"

[target.aarch64-apple-ios]
linker = "xcrun --sdk iphoneos clang"

[target.x86_64-apple-ios-sim]
linker = "xcrun --sdk iphonesimulator clang"

[alias]
b = "build"
t = "test"
c = "check"
f = "fmt --all"
l = "clippy --all-targets --all-features -- -D warnings"
d = "doc --no-deps"
```

---

## `rust-toolchain.toml`

```toml
[toolchain]
channel = "1.75.0"
components = ["rustfmt", "clippy"]
targets = [
    "x86_64-pc-windows-msvc",
    "x86_64-unknown-linux-gnu",
    "aarch64-linux-android",
    "armv7-linux-androideabi",
    "x86_64-apple-darwin",
    "aarch64-apple-darwin",
    "aarch64-apple-ios",
    "x86_64-apple-ios-sim",
]
```

---

## Работа с workspace

### 1. Добавить новый крейт

```bash
cargo new --lib crates/tensa-new
```

Затем добавить в `members` корневого `Cargo.toml`.

### 2. Добавить зависимость

```bash
cargo add tokio -p tensa-approval
```

Или вручную в `Cargo.toml`.

### 3. Обновить зависимости

```bash
cargo update
```

### 4. Проверить всё

```bash
cargo check --workspace --all-targets --all-features
cargo test --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
```

### 5. Собрать всё

```bash
cargo build --workspace --release
```

---

## Частые ошибки

### 1. Циклические зависимости

**Плохо:** `tensa-approval` зависит от `tensa-sync`, а `tensa-sync` от `tensa-approval`.

**Хорошо:** вынести общее в `tensa-core`.

### 2. Разные версии зависимостей

**Плохо:** `tokio = "1.35"` в одном крейте, `tokio = "1.36"` в другом.

**Хорошо:** `tokio.workspace = true` везде.

### 3. Дублирование кода

**Плохо:** копировать типы между крейтами.

**Хорошо:** вынести в `tensa-core`.

### 4. Слишком много зависимостей

**Плохо:** крейт зависит от 50 других.

**Хорошо:** минимизировать зависимости.

### 5. Игнорирование фич

**Плохо:** всегда собирать с `--all-features`.

**Хорошо:** использовать фичи для опциональных зависимостей.

### 6. Отсутствие `Cargo.lock` в git

**Плохо:** `Cargo.lock` в `.gitignore`.

**Хорошо:** `Cargo.lock` в git (для приложений).

### 7. Игнорирование `cargo check`

**Плохо:** `cargo build` на каждый чих.

**Хорошо:** `cargo check` для быстрой проверки.

---

## Чек-лист работы с workspace

- [ ] Новый крейт добавлен в `members`.
- [ ] Версии зависимостей — из workspace.
- [ ] Нет циклических зависимостей.
- [ ] Нет дублирования кода.
- [ ] `cargo check --workspace` проходит.
- [ ] `cargo test --workspace` проходит.
- [ ] `cargo clippy --workspace` проходит.
- [ ] `cargo fmt --all -- --check` проходит.
- [ ] `cargo build --workspace --release` проходит.

---

## Следующие файлы

- `07-ci.md` — CI.
```

