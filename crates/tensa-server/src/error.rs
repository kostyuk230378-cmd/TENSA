//! Ошибки HTTP-слоя (Контракт 12).
//!
//! `ServerError` — единая ошибка обработчиков: преобразуется в HTTP-ответ
//! с JSON-телом `{"error": "..."}` и статусом, соответствующим причине.
//! Паник в прод-коде нет: любая ошибка возвращается значением.

use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use tensa_approval::ApprovalError;
use tensa_core::CoreError;
use thiserror::Error;

/// Удобный алиас результата крейта.
pub type Result<T> = std::result::Result<T, ServerError>;

/// Ошибки HTTP-бэкенда PWA.
#[derive(Debug, Error)]
pub enum ServerError {
    /// Нет заголовка `Authorization` или неверный Bearer-токен.
    #[error("unauthorized: {0}")]
    Unauthorized(String),

    /// Некорректный запрос: путь, query-параметры или тело.
    #[error("bad request: {0}")]
    BadRequest(String),

    /// Объект не найден.
    #[error("not found: {0}")]
    NotFound(String),

    /// Конфликт состояния (недопустимый переход статуса действия и т. п.).
    #[error("conflict: {0}")]
    Conflict(String),

    /// Маршрут Контракта 12 существует, но не реализован в MVP (501).
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// Ошибка хранилища tensa-core (заметки, конфигурация).
    #[error("core error: {0}")]
    Core(#[from] CoreError),

    /// Ошибка хранилища одобрений.
    #[error("approval error: {0}")]
    Approval(#[from] ApprovalError),

    /// Ошибка ввода-вывода (токен, статика).
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Ошибка сериализации JSON (события SSE, тела ответов).
    #[error("serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

impl ServerError {
    /// HTTP-статус, соответствующий ошибке.
    ///
    /// Ошибки ядра и БД отдаются как 500: подробности уходят в лог и в тело
    /// ответа, клиенту не раскрываются внутренние детали хранения.
    pub fn status(&self) -> StatusCode {
        match self {
            ServerError::Unauthorized(_) => StatusCode::UNAUTHORIZED,
            ServerError::BadRequest(_) => StatusCode::BAD_REQUEST,
            ServerError::NotFound(_) => StatusCode::NOT_FOUND,
            ServerError::Conflict(_) => StatusCode::CONFLICT,
            ServerError::NotImplemented(_) => StatusCode::NOT_IMPLEMENTED,
            ServerError::Core(CoreError::NotFound(_)) => StatusCode::NOT_FOUND,
            ServerError::Core(CoreError::Config(_)) => StatusCode::BAD_REQUEST,
            ServerError::Approval(ApprovalError::NotFound(_)) => StatusCode::NOT_FOUND,
            ServerError::Approval(
                ApprovalError::InvalidTransition { .. } | ApprovalError::Expired(_),
            ) => StatusCode::CONFLICT,
            ServerError::Approval(
                ApprovalError::KindDisabled { .. } | ApprovalError::MaxPendingReached { .. },
            ) => StatusCode::BAD_REQUEST,
            ServerError::Core(_)
            | ServerError::Approval(_)
            | ServerError::Io(_)
            | ServerError::Json(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

/// Тело ответа об ошибке (публичный API, serde).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ErrorBody {
    /// Человекочитаемое описание ошибки.
    pub error: String,
}

impl IntoResponse for ServerError {
    fn into_response(self) -> Response {
        let status = self.status();
        if status == StatusCode::INTERNAL_SERVER_ERROR {
            tracing::error!(error = %self, "внутренняя ошибка обработчика");
        } else {
            tracing::debug!(error = %self, "запрос отклонён");
        }
        let body = Json(ErrorBody {
            error: self.to_string(),
        });
        let mut response = (status, body).into_response();
        if status == StatusCode::UNAUTHORIZED {
            response
                .headers_mut()
                .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_mapping() {
        assert_eq!(
            ServerError::Unauthorized("нет токена".into()).status(),
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            ServerError::NotImplemented("GET /query".into()).status(),
            StatusCode::NOT_IMPLEMENTED
        );
        assert_eq!(
            ServerError::Core(CoreError::NotFound("note".into())).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServerError::Approval(ApprovalError::NotFound("action".into())).status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            ServerError::Approval(ApprovalError::Expired("action".into())).status(),
            StatusCode::CONFLICT
        );
    }
}
