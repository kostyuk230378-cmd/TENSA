//! Маршруты HTTP API (базовый путь `/api/v1` — из Контракта 12, `servers.url`).
//!
//! Весь API-роутер закрыт Bearer-авторизацией: middleware навешивается в
//! [`crate::router`] на результат [`api`]. Статика PWA (см. [`static_files`])
//! публична — браузер не прикладывает заголовок при навигации.

pub mod approvals;
pub mod apps;
pub mod brain;
pub mod calendar;
pub mod contacts;
pub mod email;
pub mod email_imap;
pub mod email_setup;
pub mod events;
pub mod files;
pub mod first_run;
pub mod health;
pub mod notes;
pub mod query;
pub mod speech_models;
pub mod static_files;
pub mod stubs;
pub mod telegram_setup;
pub mod voice;

use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};
use axum::Router;

use crate::models_upload;
use crate::state::AppState;

/// API-роутер MVP: реализованные маршруты + заглушки 501 остальных.
///
/// Статические сегменты (`/notes/search`) в axum имеют приоритет над
/// параметрическими (`/notes/:id`) независимо от порядка объявления.
pub fn api() -> Router<AppState> {
    Router::new()
        // Состояние и версия ядра.
        .route("/health", get(health::health))
        .route("/version", get(health::version))
        .route("/server/info", get(health::server_info))
        // Очередь одобрения (Контракт 1).
        .route("/approvals", get(approvals::list))
        .route("/approvals/:id", get(approvals::get))
        .route("/approvals/:id/approve", post(approvals::approve))
        .route("/approvals/:id/reject", post(approvals::reject))
        // Заметки (Контракт 3).
        .route("/notes", get(notes::list).post(notes::create))
        .route("/notes/search", get(notes::search))
        .route(
            "/notes/:id",
            get(notes::get).put(notes::update).delete(notes::delete),
        )
        // Контакты (Контракт 3).
        .route("/contacts", get(contacts::list).post(contacts::create))
        .route("/contacts/search", get(contacts::search))
        .route(
            "/contacts/:id",
            get(contacts::get)
                .put(contacts::update)
                .delete(contacts::delete),
        )
        // Приложения ОС (Контракт 5).
        .route("/apps", get(apps::list))
        .route("/apps/scan", post(apps::scan))
        .route("/apps/launch", post(apps::launch))
        .route("/apps/open-url", post(apps::open_url))
        // Почта: IMAP-чтение и отправка (Контракт 6).
        .route("/email/accounts/:id/connect", post(email_imap::connect))
        .route(
            "/email/accounts/:id/disconnect",
            post(email_imap::disconnect),
        )
        .route("/email/accounts/:id/folders", get(email_imap::folders))
        .route("/email/accounts/:id/emails", get(email_imap::emails))
        .route("/email/accounts/:id/search", get(email_imap::search))
        .route("/email/send", post(email_imap::send))
        // Первый запуск (Контракт 12, доп. ТЗ tensa-core).
        .route("/first-run", get(first_run::status))
        .route("/first-run/step/language", post(first_run::step_language))
        .route(
            "/first-run/step/introduction",
            post(first_run::step_introduction),
        )
        .route("/first-run/step/models", post(first_run::step_models))
        .route("/first-run/step/telegram", post(first_run::step_telegram))
        .route("/first-run/step/email", post(first_run::step_email))
        .route("/first-run/step/whatsapp", post(first_run::step_whatsapp))
        .route("/first-run/step/tailscale", post(first_run::step_tailscale))
        .route("/first-run/step/finish", post(first_run::step_finish))
        // Поток событий (SSE).
        .route("/events", get(events::events))
        // Запросы к моделям через ядро (Контракт 12): облако/локальная GGUF.
        .route("/query", post(query::query))
        // Календарь (Контракт 7): события, конфликты, ICS.
        .route(
            "/calendar/events",
            get(calendar::list).post(calendar::create),
        )
        .route("/calendar/conflicts", get(calendar::conflicts))
        .route("/calendar/ics", get(calendar::ics))
        .route("/calendar/events/:id", delete(calendar::remove))
        // Почта: аккаунты из конфига (отправка — контур tensa-tools).
        .route(
            "/email/accounts",
            get(email::accounts).post(email_setup::save),
        )
        .route("/email/config", get(email_setup::config))
        .route("/email/accounts/:id", delete(email_setup::remove))
        .route("/telegram/status", get(telegram_setup::status))
        .route("/telegram", post(telegram_setup::save))
        .route("/telegram/detect", post(telegram_setup::detect))
        // Локальные файлы: индекс и поиск (секретарь ищет документы сам).
        .route("/files/status", get(files::status))
        .route("/files/index", post(files::index))
        .route("/files/search", get(files::search))
        .route("/files/open", post(files::open_file))
        .route("/files/download", get(files::download))
        // Модель эмбеддингов: статус и приём файлов из APK.
        // Локальное распознавание речи (whisper) — WAV 16 кГц моно.
        .route(
            "/voice/transcribe",
            post(voice::transcribe).layer(DefaultBodyLimit::max(32 * 1024 * 1024)),
        )
        // Модель ассистента: локальная Gemma / OpenRouter.
        .route("/brain/status", get(brain::status))
        .route("/brain/local/download", post(brain::download_local))
        .route("/brain/local/activate", post(brain::activate_local))
        .route("/brain/cloud", post(brain::connect_cloud))
        .route("/brain/cloud/activate", post(brain::activate_cloud))
        // Модели распознавания речи: каталог, загрузка, активация.
        .route("/models/speech", get(speech_models::list))
        .route("/models/speech/:id/download", post(speech_models::download))
        .route("/models/speech/:id/activate", post(speech_models::activate))
        .route("/models/embedder", get(models_upload::status))
        .route(
            "/models/embedder/:file",
            post(models_upload::upload).layer(DefaultBodyLimit::max(335 * 1024 * 1024)),
        )
        // Остальные маршруты Контракта 12 — 501 Not Implemented.
        .merge(stubs::routes())
}
