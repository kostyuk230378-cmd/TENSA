//! Команды CLI: контекст выполнения и диспетчеризация (ТЗ tensa-cli).
//!
//! [derived] Контекст как тип в ТЗ не описан: CLI не хранит состояние между
//! запусками (каждый вызов — процесс), `Ctx` лишь выводит пути из `-c/--config`
//! и лениво открывает хранилища.

pub mod approval;
pub mod apps;
pub mod benchmark;
pub mod calendar;
pub mod config;
pub mod contacts;
pub mod email;
pub mod health;
pub mod init;
pub mod memory;
pub mod nodes;
pub mod notes;
pub mod query;
pub mod start;
pub mod status;
pub mod stop;
pub mod sync;
pub mod transcribe;
pub mod version;

use std::path::PathBuf;

use crate::cli::{Cli, Command};
use crate::error::{CliError, Result};
use crate::output::Output;
use tensa_core::{Config, TensaPaths};

/// Контекст выполнения: пути, путь к конфигу, режим вывода.
pub struct Ctx {
    /// Пути Tensa (корень — каталог config.toml).
    pub paths: TensaPaths,
    /// Путь к config.toml (из `-c/--config` или по умолчанию `~/.tensa/`).
    pub config_path: PathBuf,
    /// Режим вывода (`--json`, `--quiet`).
    pub output: Output,
}

impl Ctx {
    /// Собрать контекст из разобранных аргументов.
    pub fn from_cli(cli: &Cli) -> Self {
        let default_paths = TensaPaths::new();
        let config_path = cli
            .config
            .clone()
            .unwrap_or_else(|| default_paths.config_file());
        let root = config_path
            .parent()
            .map(|parent| parent.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            paths: TensaPaths::with_root(root),
            config_path,
            output: Output::new(cli.json, cli.quiet),
        }
    }

    /// Загрузить конфигурацию; понятная ошибка, если ядро не инициализировано.
    pub fn config(&self) -> Result<Config> {
        if !self.config_path.exists() {
            return Err(CliError::Message(format!(
                "конфиг не найден: {} — выполните `tensa init`",
                self.config_path.display()
            )));
        }
        Ok(Config::load(&self.config_path)?)
    }

    /// Открыть хранилище одобрений (`<root>/approval.db`).
    pub async fn approvals(&self) -> Result<tensa_approval::SqliteApprovalStore> {
        let config = self.config()?;
        let store = tensa_approval::SqliteApprovalStore::new(
            self.paths.approval_db(),
            tensa_approval::ApprovalConfig::from(&config.approval),
            config.general.node_id.clone(),
        )
        .await?;
        Ok(store)
    }

    /// Открыть хранилище заметок (`<root>/notes.db`).
    pub async fn notes(&self) -> Result<tensa_core::SqliteNotesStore> {
        let config = self.config()?;
        let store = tensa_core::SqliteNotesStore::new(
            &self.paths.notes_db(),
            config.general.node_id.clone(),
        )
        .await?;
        Ok(store)
    }

    /// Открыть хранилище контактов (`<root>/contacts.db`).
    ///
    /// [derived] `contacts.db` не перечислен в TensaPaths (Контракт 4 задаёт
    /// лишь approval/notes/memory/models) — имя файла выбирается здесь.
    pub async fn contacts(&self) -> Result<tensa_core::SqliteContactsStore> {
        let config = self.config()?;
        let db = self.paths.root().join("contacts.db");
        let store =
            tensa_core::SqliteContactsStore::new(&db, config.general.node_id.clone()).await?;
        Ok(store)
    }

    /// Открыть реестр узлов (`<root>/nodes.db`).
    ///
    /// [derived] Имя файла — по аналогии с остальными хранилищами корня.
    pub async fn nodes(&self) -> Result<tensa_nodes::SqliteNodeRegistry> {
        let db = self.paths.root().join("nodes.db");
        let registry = tensa_nodes::SqliteNodeRegistry::new(&db).await?;
        Ok(registry)
    }
}

/// Диспетчер команд верхнего уровня.
pub async fn dispatch(cli: Cli) -> Result<()> {
    let ctx = Ctx::from_cli(&cli);
    match &cli.command {
        Command::Init(args) => init::run(&ctx, args).await,
        Command::Start => start::run(&ctx).await,
        Command::Stop => stop::run(&ctx).await,
        Command::Status => status::run(&ctx).await,
        Command::Health => health::run(&ctx).await,
        Command::Config(args) => config::run(&ctx, args).await,
        Command::Approval(args) => approval::run(&ctx, args).await,
        Command::Memory(args) => memory::run(&ctx, args).await,
        Command::Notes(args) => notes::run(&ctx, args).await,
        Command::Contacts(args) => contacts::run(&ctx, args).await,
        Command::Email(args) => email::run(&ctx, args).await,
        Command::Calendar(args) => calendar::run(&ctx, args).await,
        Command::Apps(args) => apps::run(&ctx, args).await,
        Command::Nodes(args) => nodes::run(&ctx, args).await,
        Command::Sync(args) => sync::run(&ctx, args).await,
        Command::Query(args) => query::run(&ctx, args).await,
        Command::Transcribe(args) => transcribe::run(&ctx, args).await,
        Command::Benchmark(args) => benchmark::run(&ctx, args).await,
        Command::Version => version::run(&ctx).await,
    }
}

/// Unix-timestamp → «YYYY-MM-DD HH:MM» (UTC); пусто при выходе за диапазон.
pub fn ts(secs: i64) -> String {
    chrono::DateTime::<chrono::Utc>::from_timestamp(secs, 0)
        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
        .unwrap_or_default()
}

/// Имя enum-значения в serde-представлении (snake_case, как в контрактах).
pub fn enum_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value)
        .ok()
        .and_then(|json| json.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Укоротить длинное значение для таблицы.
pub fn short(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        let mut text: String = value.chars().take(max).collect();
        text.push('…');
        text
    }
}
