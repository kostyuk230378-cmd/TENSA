//! Bearer-токен PWA и middleware авторизации.
//!
//! Контракт 12 задаёт `security: BearerAuth`; ТЗ apps/pwa уточняет: токен
//! генерируется при первом запуске ядра, показывается владельцу в UI десктопа
//! и проверяется на каждом запросе. Файл токена — `<root>/pwa_token`
//! (корень данных — каталог `config.toml`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::http::header::AUTHORIZATION;
use axum::middleware::Next;
use axum::response::Response;
use rand::RngCore;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Имя файла токена в корне данных (`<root>/pwa_token`).
pub const TOKEN_FILE: &str = "pwa_token";

/// Длина токена в байтах: 32 случайных байта → 64 hex-символа.
pub const TOKEN_BYTES: usize = 32;

/// Bearer-токен PWA.
///
/// Newtype скрывает значение: оно не попадает в `Debug`, а значит и в логи;
/// проверка доступа выполняется через [`PwaToken::matches`].
#[derive(Clone)]
pub struct PwaToken(Arc<String>);

impl PwaToken {
    /// Токен с известным значением (чтение из файла, тесты).
    pub fn new(value: impl Into<String>) -> Self {
        Self(Arc::new(value.into()))
    }

    /// Свежий токен: 32 случайных байта из ОС, hex.
    pub fn generate() -> Self {
        Self(Arc::new(generate_token()))
    }

    /// Значение токена — для записи в файл и показа владельцу в UI десктопа
    /// (ТЗ apps/pwa: QR или текст). В логи не попадает.
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    /// Проверка предъявленного значения: побайтовое сравнение без раннего
    /// выхода (длина сверяется первой — она не секрет).
    pub fn matches(&self, provided: &str) -> bool {
        let expected = self.0.as_bytes();
        let provided = provided.as_bytes();
        if expected.len() != provided.len() {
            return false;
        }
        let mut diff = 0u8;
        for (a, b) in expected.iter().zip(provided.iter()) {
            diff |= a ^ b;
        }
        diff == 0
    }
}

impl std::fmt::Debug for PwaToken {
    /// Значение токена не печатается.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PwaToken(<скрыто>)")
    }
}

/// Путь к файлу токена в корне данных.
pub fn token_path(root: &Path) -> PathBuf {
    root.join(TOKEN_FILE)
}

/// Сгенерировать токен: 32 случайных байта из ОС в hex-виде.
pub fn generate_token() -> String {
    let mut bytes = [0u8; TOKEN_BYTES];
    rand::rng().fill_bytes(&mut bytes);
    to_hex(&bytes)
}

/// Hex-кодирование (без внешних зависимостей).
fn to_hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(DIGITS[usize::from(byte >> 4)] as char);
        out.push(DIGITS[usize::from(byte & 0x0f)] as char);
    }
    out
}

/// Прочитать Bearer-токен из `<root>/pwa_token`; если файла нет — создать.
///
/// В лог уходит только путь файла: значение токена не логируется.
pub async fn load_or_create_token(root: &Path) -> Result<PwaToken> {
    let path = token_path(root);
    if let Ok(raw) = tokio::fs::read_to_string(&path).await {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            tracing::debug!(path = %path.display(), "PWA-токен прочитан из файла");
            return Ok(PwaToken::new(trimmed));
        }
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let access = PwaToken::generate();
    tokio::fs::write(&path, format!("{}\n", access.expose())).await?;
    restrict_file_mode(&path)?;
    tracing::info!(path = %path.display(), "PWA-токен создан (значение не логируется)");
    Ok(access)
}

/// Права `0600` на файл токена (только владелец). На не-Unix — no-op.
#[cfg(unix)]
fn restrict_file_mode(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

/// Права `0600` на файл токена (только владелец). На не-Unix — no-op.
#[cfg(not(unix))]
fn restrict_file_mode(_path: &Path) -> Result<()> {
    Ok(())
}

/// Middleware авторизации: пропускает запрос только с валидным
/// `Authorization: Bearer <token>`; иначе — 401 (Контракт 12).
pub async fn require_bearer(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Result<Response> {
    let provided = request
        .headers()
        .get(AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bearer_value)
        .map(str::to_string);

    match provided {
        Some(value) if state.token_matches(&value) => Ok(next.run(request).await),
        Some(_) => {
            tracing::warn!(path = %request.uri().path(), "запрос с неверным Bearer-токеном");
            Err(ServerError::Unauthorized("неверный токен".into()))
        }
        None => Err(ServerError::Unauthorized(
            "требуется заголовок Authorization: Bearer <token>".into(),
        )),
    }
}

/// Разбор значения заголовка `Authorization`: схема `Bearer` без учёта регистра.
fn bearer_value(header: &str) -> Option<&str> {
    let (scheme, value) = header.split_once(' ')?;
    let value = value.trim();
    if scheme.eq_ignore_ascii_case("bearer") && !value.is_empty() {
        Some(value)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn token_file_is_created_and_reused() {
        let root =
            std::env::temp_dir().join(format!("tensa-server-token-{}", uuid::Uuid::new_v4()));
        let created = load_or_create_token(&root).await.expect("создание токена");
        assert_eq!(created.expose().len(), TOKEN_BYTES * 2);
        assert!(created.expose().chars().all(|c| c.is_ascii_hexdigit()));
        assert!(token_path(&root).is_file());

        let again = load_or_create_token(&root).await.expect("повторное чтение");
        assert_eq!(created.expose(), again.expose());
        assert!(created.matches(again.expose()));

        std::fs::remove_dir_all(root).ok();
    }

    #[test]
    fn generated_tokens_differ() {
        let first = generate_token();
        let second = generate_token();
        assert_ne!(first, second);
        assert_eq!(first.len(), TOKEN_BYTES * 2);
    }

    #[test]
    fn bearer_header_parsing() {
        assert_eq!(bearer_value("Bearer abc"), Some("abc"));
        assert_eq!(bearer_value("bearer abc"), Some("abc"));
        assert_eq!(bearer_value("Basic abc"), None);
        assert_eq!(bearer_value("Bearer "), None);
        assert_eq!(bearer_value("abc"), None);
    }

    #[test]
    fn matching_is_exact() {
        let access = PwaToken::generate();
        let value = access.expose().to_string();
        assert!(access.matches(&value));
        assert!(!access.matches(&format!("{value}0")));
        assert!(!access.matches(value.get(1..).unwrap_or_default()));
    }

    #[test]
    fn debug_hides_value() {
        let access = PwaToken::generate();
        let value = access.expose().to_string();
        assert!(!format!("{access:?}").contains(&value));
    }
}
