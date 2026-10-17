//! `POST /api/v1/voice/transcribe` — локальное распознавание речи (whisper).

use axum::body::Bytes;
use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Ответ распознавания.
#[derive(Debug, Serialize)]
pub struct Transcript {
    /// Распознанный текст.
    pub text: String,
}

/// `POST /voice/transcribe` — принять WAV и вернуть текст.
pub async fn transcribe(State(state): State<AppState>, body: Bytes) -> Result<Json<Transcript>> {
    if body.is_empty() {
        return Err(ServerError::BadRequest("пустое аудио".to_string()));
    }
    let text = state
        .voice
        .transcribe(&body)
        .await
        .map_err(|error| ServerError::Unavailable(error.to_string()))?;
    Ok(Json(Transcript { text }))
}
