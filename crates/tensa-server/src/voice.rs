//! Локальное распознавание речи через отдельный `tensa-whisper.exe`.
//!
//! llama.cpp и whisper.cpp содержат одинаковые символы ggml, поэтому whisper
//! живёт в своём процессе: сервер пишет WAV во временный файл и запускает
//! helper. Модель — small q5_1 в `<root>/models` или рядом с установленным exe.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

/// Слот постоянного ASR-worker (разделяется между потоками).
type WorkerSlot = Arc<Mutex<Option<Arc<Mutex<AsrWorker>>>>>;

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
    /// Постоянный worker для Parakeet/Canary/Nemotron: модель не грузится заново.
    worker: WorkerSlot,
}

/// Постоянный процесс `tensa-asr --worker`: модель загружена один раз.
struct AsrWorker {
    child: Child,
    input: ChildStdin,
    output: BufReader<ChildStdout>,
    model: PathBuf,
}

impl AsrWorker {
    /// Запустить worker и дождаться `READY` после загрузки модели.
    fn start(helper: &Path, model: &Path) -> std::result::Result<Self, String> {
        let mut child = Command::new(helper)
            .arg("--worker")
            .arg(model)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|error| format!("запуск ASR-worker: {error}"))?;
        let input = child
            .stdin
            .take()
            .ok_or_else(|| "ASR-worker: нет stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "ASR-worker: нет stdout".to_string())?;
        let mut output = BufReader::new(stdout);
        let mut ready = String::new();
        let read = output
            .read_line(&mut ready)
            .map_err(|error| format!("ASR-worker: чтение READY: {error}"))?;
        if read == 0 || ready.trim() != "READY" {
            let _ = child.kill();
            let _ = child.wait();
            let message = parse_worker_error(&ready)
                .unwrap_or_else(|| "ASR-worker не загрузил модель".to_string());
            return Err(message);
        }
        Ok(Self {
            child,
            input,
            output,
            model: model.to_path_buf(),
        })
    }

    /// Жив ли процесс.
    fn healthy(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    /// Распознать WAV через уже загруженную модель.
    fn transcribe(&mut self, wav: &Path) -> std::result::Result<String, String> {
        let line = wav.to_string_lossy();
        writeln!(self.input, "{line}")
            .and_then(|()| self.input.flush())
            .map_err(|error| format!("ASR-worker: запись: {error}"))?;
        let mut response = String::new();
        let read = self
            .output
            .read_line(&mut response)
            .map_err(|error| format!("ASR-worker: ответ: {error}"))?;
        if read == 0 {
            return Err("ASR-worker завершился".to_string());
        }
        let value: serde_json::Value = serde_json::from_str(response.trim())
            .map_err(|error| format!("ASR-worker: протокол: {error}"))?;
        if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
            return value
                .get("text")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
                .ok_or_else(|| "ASR-worker: пустой ответ".to_string());
        }
        Err(value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("ASR-worker: ошибка распознавания")
            .to_string())
    }
}

impl Drop for AsrWorker {
    fn drop(&mut self) {
        let _ = self.input.write_all(b"QUIT\n");
        let _ = self.input.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn parse_worker_error(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line.trim()).ok()?;
    if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        return value
            .get("error")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
    }
    None
}

/// Взять готовый worker или запустить/перезапустить его под нужную модель.
fn ensure_worker(
    slot: &WorkerSlot,
    helper: &Path,
    model: &Path,
) -> std::result::Result<Arc<Mutex<AsrWorker>>, String> {
    let mut slot = slot
        .lock()
        .map_err(|_| "ASR-worker: слот занят".to_string())?;
    let stale = match slot.as_ref() {
        Some(worker) => {
            let mut worker = worker
                .lock()
                .map_err(|_| "ASR-worker: процесс занят".to_string())?;
            worker.model != model || !worker.healthy()
        }
        None => false,
    };
    if stale {
        *slot = None;
    }
    if slot.is_none() {
        let worker = AsrWorker::start(helper, model)?;
        *slot = Some(Arc::new(Mutex::new(worker)));
    }
    slot.as_ref()
        .cloned()
        .ok_or_else(|| "ASR-worker не создан".to_string())
}

/// Модели, которые обслуживает transcribe.cpp (`tensa-asr`).
fn needs_transcribe_cpp(model: &Path) -> bool {
    let name = model
        .file_name()
        .map(|value| value.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    name.contains("parakeet") || name.contains("canary") || name.contains("nemotron")
}

/// Является ли helper постоянным worker-процессом `tensa-asr`.
fn is_asr_helper(path: &Path) -> bool {
    path.file_name()
        .map(|value| value.to_string_lossy().to_lowercase().contains("tensa-asr"))
        .unwrap_or(false)
}

impl VoiceService {
    /// Сервис для корня данных: модель в models/whisper или models.
    pub fn new(root: &Path) -> Self {
        let roots = [install_models_dir().join("whisper"), install_models_dir()];
        // По умолчанию квантованная small.
        let names = ["ggml-small-q5_1.bin", "ggml-small.bin"];
        let model = roots
            .iter()
            .flat_map(|dir| names.iter().map(move |name| dir.join(name)))
            .find(|path| path.is_file())
            .unwrap_or_else(|| {
                root.join("models")
                    .join("whisper")
                    .join("ggml-small-q5_1.bin")
            });
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
            worker: Arc::new(Mutex::new(None)),
        }
    }

    /// Заглушка без модели (тесты и состояние без голоса).
    pub fn disabled() -> Self {
        Self {
            model: PathBuf::new(),
            root: PathBuf::new(),
            helper: PathBuf::new(),
            worker: Arc::new(Mutex::new(None)),
        }
    }

    /// Прогреть ASR-worker в фоне, чтобы первый запрос не платил за загрузку модели.
    pub fn preload(&self) {
        let model = self.resolve_model();
        let helper = self.helper_for(&model);
        if !is_asr_helper(&helper) || !model.is_file() {
            return;
        }
        if !helper.is_file() {
            return;
        }
        let slot = Arc::clone(&self.worker);
        std::thread::spawn(move || {
            if let Err(error) = ensure_worker(&slot, &helper, &model) {
                tracing::warn!(error = %error, "ASR worker не прогрет");
            }
        });
    }

    /// Выбрать активную модель: asr-active.txt, иначе приоритет small.
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

    /// Helper для модели: предпочитаем постоянный `tensa-asr` (он умеет и Whisper).
    fn helper_for(&self, model: &Path) -> PathBuf {
        // Основной путь: `tensa-asr --worker` для Whisper и Parakeet/Nemotron.
        if let Some(path) = std::env::var_os("TENSA_ASR").map(PathBuf::from) {
            if path.is_file() {
                return path;
            }
        }
        if let Some(exe) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        {
            let candidate = exe.join("tensa-asr.exe");
            if candidate.is_file() {
                return candidate;
            }
        }
        // Fallback для старых установок без tensa-asr — отдельный whisper.cpp.
        let asr = needs_transcribe_cpp(model);
        let env_name = if asr { "TENSA_ASR" } else { "TENSA_WHISPER" };
        if let Some(path) = std::env::var_os(env_name).map(PathBuf::from) {
            if path.is_file() {
                return path;
            }
        }
        if let Some(exe) = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
        {
            let candidate = exe.join(if asr {
                "tensa-asr.exe"
            } else {
                "tensa-whisper.exe"
            });
            if candidate.is_file() {
                return candidate;
            }
        }
        if asr {
            PathBuf::from("tensa-asr.exe")
        } else {
            self.helper.clone()
        }
    }

    /// Распознать WAV (PCM16; helper сам приводит к 16 кГц моно).
    pub async fn transcribe(&self, wav: &[u8]) -> Result<String> {
        let model = self.resolve_model();
        let helper = self.helper_for(&model);
        if model.as_os_str().is_empty() || !model.is_file() {
            return Err(VoiceError::Message(
                "модель whisper не найдена: models/whisper/ggml-small-q5_1.bin \
                 (скачать в Настройках → Модели распознавания речи)"
                    .to_string(),
            ));
        }
        if helper.as_os_str().is_empty() || !helper.is_file() {
            return Err(VoiceError::Message(format!(
                "не найден {} рядом с сервером",
                helper.file_name().unwrap_or_default().to_string_lossy()
            )));
        }

        let path = temp_wav_path();
        std::fs::write(&path, wav)
            .map_err(|error| VoiceError::Message(format!("временный WAV: {error}")))?;
        let asr = is_asr_helper(&helper);
        let text = if asr {
            let slot = Arc::clone(&self.worker);
            let helper = helper.clone();
            let model = model.clone();
            let audio = path.clone();
            tokio::task::spawn_blocking(move || {
                let worker = ensure_worker(&slot, &helper, &model)?;
                let mut worker = worker
                    .lock()
                    .map_err(|_| "ASR-worker: процесс занят".to_string())?;
                worker.transcribe(&audio)
            })
            .await
            .map_err(|error| VoiceError::Message(format!("поток: {error}")))?
            .map_err(VoiceError::Message)?
        } else {
            let helper = helper.clone();
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
                let _ = std::fs::remove_file(&path);
                return Err(VoiceError::Message(message));
            }
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        };
        let _ = std::fs::remove_file(&path);
        if text.is_empty() {
            return Err(VoiceError::Message("речь не распознана".to_string()));
        }
        Ok(text)
    }
}

/// Каталог моделей рядом с exe установки (fallback — текущий).
fn install_models_dir() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("models")))
        .unwrap_or_else(|| PathBuf::from("models"))
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
