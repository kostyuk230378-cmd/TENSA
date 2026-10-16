//! Бинарь `tensa-server`: HTTP-бэкенд PWA (ТЗ apps/pwa, порт 8765).
//!
//! Запуск: `tensa-server [путь к config.toml]` (по умолчанию `~/.tensa/config.toml`).
//! Статика PWA раздаётся из каталога `pwa/static` (относительно рабочего
//! каталога процесса); токен доступа — `<корень данных>/pwa_token`.

use std::path::PathBuf;
use std::sync::Arc;

use tensa_approval::{ApprovalConfig, SqliteApprovalStore};
use tensa_core::{Config, SqliteNotesStore, TensaPaths};
use tensa_server::{load_or_create_token, serve, AppState};

#[tokio::main]
async fn main() {
    init_tracing();
    if let Err(error) = run().await {
        eprintln!("ошибка: {error}");
        std::process::exit(1);
    }
}

/// Собрать состояние и запустить сервер (ошибки — значением, без паник).
async fn run() -> tensa_server::Result<()> {
    let config_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| TensaPaths::new().config_file());

    let root = config_path
        .parent()
        .map(|parent| parent.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let paths = TensaPaths::with_root(root);
    paths.ensure_dirs()?;

    let config = if config_path.exists() {
        Config::load(&config_path)?
    } else {
        tracing::warn!(
            path = %config_path.display(),
            "config.toml не найден — запуск с конфигурацией по умолчанию (сначала `tensa init`)"
        );
        Config::default()
    };
    let node_id = config.general.node_id.clone();

    let approvals = SqliteApprovalStore::new(
        paths.approval_db(),
        ApprovalConfig::from(&config.approval),
        node_id.clone(),
    )
    .await?;
    let notes = SqliteNotesStore::new(&paths.notes_db(), node_id).await?;

    let access = load_or_create_token(&paths.root()).await?;

    let state = AppState::new(Arc::new(approvals), Arc::new(notes), access);
    // ТЗ apps/pwa: локальный порт ядра 8765 (Tailscale-адрес — снаружи).
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], 8765));
    tracing::info!(
        config = %config_path.display(),
        "запуск HTTP-бэкенда PWA (токен: файл pwa_token в корне данных)"
    );
    serve(addr, state).await
}

/// Логи в stderr; уровень — из `RUST_LOG`, по умолчанию info.
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
