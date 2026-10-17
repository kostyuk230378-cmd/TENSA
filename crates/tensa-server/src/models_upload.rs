//! Передача модели эмбеддингов из APK в ядро (`/api/v1/models/embedder`).
//!
//! APK носит файлы e5-small в assets; здесь он по одному кладёт их в
//! `<root>/models/multilingual-e5-small` — после этого `tensa memory` работает
//! без сети. Имена файлов — из фиксированного списка (никаких путей).

use std::path::PathBuf;

use axum::body::Bytes;
use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::error::{Result, ServerError};
use crate::state::AppState;

/// Имена файлов модели, которые принимает ядро.
pub const MODEL_FILES: [&str; 7] = [
    "model_O4.onnx",
    "model.onnx",
    "model_quantized.onnx",
    "tokenizer.json",
    "config.json",
    "special_tokens_map.json",
    "tokenizer_config.json",
];

/// Подкаталог модели в каталоге моделей.
const MODEL_DIR: &str = "multilingual-e5-small";

/// Состояние модели у ядра.
#[derive(Debug, Serialize)]
pub struct EmbedderStatus {
    /// Каталог модели.
    pub dir: String,
    /// Все обязательные файлы на месте.
    pub installed: bool,
    /// Присутствующие файлы.
    pub files: Vec<String>,
    /// Чего не хватает.
    pub missing: Vec<String>,
}

/// Результат загрузки одного файла.
#[derive(Debug, Serialize)]
pub struct Uploaded {
    /// Имя файла.
    pub file: String,
    /// Сколько байт принято.
    pub bytes: usize,
}

/// Проверить имя файла по списку (без путей).
fn validate_file(name: &str) -> Result<()> {
    if MODEL_FILES.contains(&name) {
        Ok(())
    } else {
        Err(ServerError::BadRequest(format!(
            "файл «{name}» не входит в комплект модели"
        )))
    }
}

/// Каталог модели в данных ядра.
fn model_dir(state: &AppState) -> PathBuf {
    state.data_root.join("models").join(MODEL_DIR)
}

/// `GET /models/embedder` — состояние модели.
pub async fn status(State(state): State<AppState>) -> Result<Json<EmbedderStatus>> {
    let dir = model_dir(&state);
    let mut files = Vec::new();
    for name in MODEL_FILES {
        if dir.join(name).is_file() {
            files.push(name.to_string());
        }
    }

    let has_model = files.iter().any(|file| file.starts_with("model_"));
    let mut missing: Vec<String> = [
        "tokenizer.json",
        "config.json",
        "special_tokens_map.json",
        "tokenizer_config.json",
    ]
    .into_iter()
    .filter(|name| !dir.join(name).is_file())
    .map(str::to_string)
    .collect();
    if !has_model {
        missing.push("model_*.onnx".to_string());
    }

    Ok(Json(EmbedderStatus {
        dir: dir.display().to_string(),
        installed: has_model && missing.is_empty(),
        files,
        missing,
    }))
}

/// `POST /models/embedder/:file` — принять файл модели (octet-stream).
pub async fn upload(
    State(state): State<AppState>,
    Path(name): Path<String>,
    body: Bytes,
) -> Result<Json<Uploaded>> {
    validate_file(&name)?;
    if body.is_empty() {
        return Err(ServerError::BadRequest("пустое тело файла".to_string()));
    }
    let dir = model_dir(&state);
    std::fs::create_dir_all(&dir)?;
    let target = dir.join(&name);
    let tmp = target.with_extension("part");
    std::fs::write(&tmp, &body)?;
    std::fs::rename(&tmp, &target)?;
    tracing::info!(file = %name, bytes = body.len(), "файл модели принят");
    Ok(Json(Uploaded {
        file: name,
        bytes: body.len(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_rejects_traversal() {
        assert!(validate_file("tokenizer.json").is_ok());
        assert!(validate_file("../secrets.toml").is_err());
        assert!(validate_file("model_evil.onnx").is_err());
    }
}
