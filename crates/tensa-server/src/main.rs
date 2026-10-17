//! Бинарь `tensa-server`: HTTP-бэкенд PWA (ТЗ apps/pwa, порт 8765).
//!
//! Запуск: `tensa-server [путь к config.toml]` (по умолчанию `~/.tensa/config.toml`).
//! Статика PWA раздаётся из каталога `pwa/static` (относительно рабочего
//! каталога процесса); токен доступа — `<корень данных>/pwa_token`.

use std::path::PathBuf;
use std::sync::Arc;

use tensa_approval::{ApprovalConfig, SqliteApprovalStore};
use tensa_core::{Config, Secrets, SqliteNotesStore, TensaPaths};
use tensa_server::{load_or_create_token, serve, AppState, QueryService};

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

    let secrets = Secrets::load(&paths.secrets_file())?;
    let query = QueryService::from_config(&config.brain, &secrets, &paths);
    tracing::info!(
        mode = ?query.mode(),
        cloud = query.has_cloud(),
        local = query.has_local(),
        "шлюз /query: провайдеры"
    );

    let calendar = Arc::new(tensa_server::calendar::CalendarStore::new(
        paths.root().join("calendar.json"),
    ));
    let email_accounts = tensa_server::routes::email::accounts_from_config(&config.email);
    let files = Arc::new(tensa_server::files::FilesIndex::new(
        paths.root().join("files.db"),
    ));
    let index_dirs = index_dirs(&config, &paths);
    let state = AppState::new(Arc::new(approvals), Arc::new(notes), access)
        .with_query(Arc::new(query))
        .with_calendar(calendar)
        .with_email_accounts(email_accounts)
        .with_data_root(paths.root())
        .with_config_path(config_path.clone())
        .with_files(files.clone(), index_dirs.clone())
        .with_voice(Arc::new(tensa_server::voice::VoiceService::new(
            &paths.root(),
        )));

    // Автоиндексация файлов в фоне (TENSA_INDEX_ON_START=0 отключает).
    if std::env::var("TENSA_INDEX_ON_START").as_deref() != Ok("0") && !index_dirs.is_empty() {
        let files = files.clone();
        let dirs = index_dirs.clone();
        tokio::spawn(async move {
            match files.scan(&dirs).await {
                Ok(count) => {
                    tracing::info!(indexed = count, dirs = dirs.len(), "автоиндексация файлов")
                }
                Err(error) => tracing::warn!(error = %error, "автоиндексация файлов"),
            }
        });
    }
    // Адрес сервера: по умолчанию — только локально (ТЗ apps/pwa: 127.0.0.1:8765);
    // для доступа с телефона/по Tailscale задаётся TENSA_BIND=0.0.0.0:8765.
    let raw_addr = std::env::var("TENSA_BIND").unwrap_or_else(|_| "127.0.0.1:8765".to_string());
    let addr: std::net::SocketAddr = raw_addr.parse().map_err(|error| {
        tensa_server::ServerError::BadRequest(format!("TENSA_BIND «{raw_addr}»: {error}"))
    })?;
    tracing::info!(
        config = %config_path.display(),
        bind = %addr,
        "запуск HTTP-бэкенда PWA (токен: файл pwa_token в корне данных)"
    );
    serve(addr, state).await
}

/// Каталоги индексации: `[documents] folders` из конфига или
/// Documents/Desktop/Downloads профиля пользователя.
fn index_dirs(config: &Config, paths: &TensaPaths) -> Vec<std::path::PathBuf> {
    // Переопределение для тестов и нестандартных раскладок: каталоги через ';'.
    if let Ok(raw) = std::env::var("TENSA_INDEX_DIRS") {
        let dirs: Vec<std::path::PathBuf> = raw
            .split(';')
            .map(str::trim)
            .filter(|item| !item.is_empty())
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_dir())
            .collect();
        if !dirs.is_empty() {
            return dirs;
        }
    }

    if let Some(folders) = config
        .documents
        .get("folders")
        .and_then(|value| value.as_array())
    {
        let configured: Vec<std::path::PathBuf> = folders
            .iter()
            .filter_map(|value| value.as_str())
            .map(std::path::PathBuf::from)
            .filter(|path| path.is_dir())
            .collect();
        if !configured.is_empty() {
            return configured;
        }
    }
    let _ = paths;
    // По умолчанию — все локальные диски (требование владельца: искать везде).
    tensa_server::files::default_scan_roots()
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
