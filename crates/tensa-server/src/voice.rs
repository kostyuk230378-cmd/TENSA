//! Локальное распознавание речи через отдельный `tensa-whisper.exe`.
//!
//! llama.cpp и whisper.cpp содержат одинаковые символы ggml, поэтому whisper
//! живёт в своём процессе: сервер пишет WAV во временный файл и запускает
//! helper. Модель — `<root>/models/whisper/ggml-base.bin` или `<root>/models/ggml-base.bin`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Ошибка распознавания.
#[derive(Debug, thiserror::Error)]
pub enum VoiceError {
    /// Понятное человеку сообщение.
    #[error("голос: {0}")]
    Message(String),
}

/// Результат операций распознавания.
pub type Result<T> = std::result::Result<T, VoiceError>;

/// Сервис локального распознавания речи (вызов helper-процесса).
pub struct VoiceService {
    model: PathBuf,
    helper: PathBuf,
}

impl VoiceService {
    /// Сервис для корня данных: модель в models/whisper или models.
    pub fn new(root: &Path) -> Self {
        let nested = root.join("models").join("whisper").join("ggml-base.bin");
        let flat = root.join("models").join("ggml-base.bin");
        // Установщик кладёт модель рядом с exe (models\whisper), а пользовательские
        // данные живут в ~/.tensa — поэтому смотрим оба места.
        let exe_models = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.join("models")));
        let installed = exe_models
            .as_ref()
            .map(|dir| dir.join("whisper").join("ggml-base.bin"));
        let installed_flat = exe_models.map(|dir| dir.join("ggml-base.bin"));
        let model = if nested.is_file() {
            nested
        } else if flat.is_file() {
            flat
        } else if installed.as_ref().is_some_and(|path| path.is_file()) {
            installed.unwrap_or_default()
        } else if installed_flat.as_ref().is_some_and(|path| path.is_file()) {
            installed_flat.unwrap_or_default()
        } else {
            nested
        };
        let helper = std::env::var_os("TENSA_WHISPER")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(|dir| dir.join("tensa-whisper.exe")))
            })
            .unwrap_or_else(|| PathBuf::from("tensa-whisper.exe"));
        Self { model, helper }
    }

    /// Заглушка без модели (тесты и состояние без голоса).
    pub fn disabled() -> Self {
        Self {
            model: PathBuf::new(),
            helper: PathBuf::new(),
        }
    }

    /// Распознать WAV (PCM16; helper сам приводит к 16 кГц моно).
    pub async fn transcribe(&self, wav: &[u8]) -> Result<String> {
        if self.model.as_os_str().is_empty() || !self.model.is_file() {
            return Err(VoiceError::Message(
                "модель whisper не найдена: models/whisper/ggml-base.bin \
                 (скачать: DownloadModels.cmd -Whisper)"
                    .to_string(),
            ));
        }
        if self.helper.as_os_str().is_empty() || !self.helper.is_file() {
            return Err(VoiceError::Message(
                "не найден tensa-whisper.exe рядом с сервером".to_string(),
            ));
        }

        let path = temp_wav_path();
        std::fs::write(&path, wav)
            .map_err(|error| VoiceError::Message(format!("временный WAV: {error}")))?;
        let helper = self.helper.clone();
        let model = self.model.clone();
        let audio = path.clone();
        let output = tokio::task::spawn_blocking(move || {
            Command::new(&helper)
                .arg(&model)
                .arg(&audio)
                .output()
                .map_err(|error| format!("запуск tensa-whisper: {error}"))
        })
        .await
        .map_err(|error| VoiceError::Message(format!("поток: {error}")))?;
        let _ = std::fs::remove_file(&path);

        let output = output.map_err(VoiceError::Message)?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(VoiceError::Message(stderr.trim().to_string()));
        }
        let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if text.is_empty() {
            return Err(VoiceError::Message("речь не распознана".to_string()));
        }
        Ok(text)
    }
}

/// Уникальный путь для временного WAV.
fn temp_wav_path() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_nanos())
        .unwrap_or(0);
    std::env::temp_dir().join(format!("tensa-voice-{}-{nanos}.wav", std::process::id()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn disabled_reports_model_error() {
        let service = VoiceService::disabled();
        let error = service
            .transcribe(b"RIFF")
            .await
            .expect_err("должна быть ошибка");
        assert!(error.to_string().contains("модель whisper"));
    }

    #[test]
    fn temp_paths_are_unique() {
        assert_ne!(temp_wav_path(), temp_wav_path());
    }
}
