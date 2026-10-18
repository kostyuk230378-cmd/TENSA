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
    root: PathBuf,
    helper: PathBuf,
}

impl VoiceService {
    /// Сервис для корня данных: модель в models/whisper или models.
    pub fn new(root: &Path) -> Self {
        let mut roots = vec![root.join("models").join("whisper"), root.join("models")];
        if let Some(exe_dir) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|dir| dir.to_path_buf()))
        {
            roots.push(exe_dir.join("models").join("whisper"));
            roots.push(exe_dir.join("models"));
        }
        // Сначала квантованная small (точнее для русского), затем base.
        let names = ["ggml-small-q5_1.bin", "ggml-small.bin", "ggml-base.bin"];
        let model = roots
            .iter()
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .find(|path| path.is_file())
            .unwrap_or_else(|| root.join("models").join("whisper").join("ggml-base.bin"));
        let helper = std::env::var_os("TENSA_WHISPER")
            .map(PathBuf::from)
            .or_else(|| {
                std::env::current_exe()
                    .ok()
                    .and_then(|exe| exe.parent().map(|dir| dir.join("tensa-whisper.exe")))
            })
            .unwrap_or_else(|| PathBuf::from("tensa-whisper.exe"));
        Self {
            model,
            root: root.to_path_buf(),
            helper,
        }
    }

    /// Заглушка без модели (тесты и состояние без голоса).
    pub fn disabled() -> Self {
        Self {
            model: PathBuf::new(),
            root: PathBuf::new(),
            helper: PathBuf::new(),
        }
    }

    /// Выбрать активную модель: asr-active.txt, иначе приоритет small/base.
    fn resolve_model(&self) -> PathBuf {
        if self.root.as_os_str().is_empty() {
            return self.model.clone();
        }
        let active = std::fs::read_to_string(self.root.join("models").join("asr-active.txt"))
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(active) = active {
            let mut roots = vec![
                self.root.join("models"),
                self.root.join("models").join("whisper"),
            ];
            if let Some(exe) = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf))
            {
                roots.push(exe.join("models"));
                roots.push(exe.join("models").join("whisper"));
            }
            for dir in roots {
                let candidate = dir.join(&active);
                if candidate.is_file() {
                    return candidate;
                }
            }
        }
        if self.model.is_file() {
            return self.model.clone();
        }
        self.model.clone()
    }

    /// Распознать WAV (PCM16; helper сам приводит к 16 кГц моно).
    pub async fn transcribe(&self, wav: &[u8]) -> Result<String> {
        let model = self.resolve_model();
        if model.as_os_str().is_empty() || !model.is_file() {
            return Err(VoiceError::Message(
                "модель whisper не найдена: models/whisper/ggml-small-q5_1.bin \
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
            // whisper.cpp пишет в stderr свой шум; пользователю нужна последняя причина.
            let mut lines = stderr.lines().map(str::trim).filter(|line| {
                !line.is_empty()
                    && !line.starts_with("whisper_")
                    && !line.starts_with("ggml_")
                    && !line.starts_with("register_")
                    && !line.starts_with("load:")
                    && !line.starts_with("system_info")
                    && !line.contains("buffer")
            });
            let message = lines
                .next_back()
                .unwrap_or("не удалось распознать речь")
                .to_string();
            return Err(VoiceError::Message(message));
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
