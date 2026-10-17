//! Нативный движок llama.cpp (фича `native`; TASK-ENV-001, ADR-003).
//!
//! Здесь сосредоточены все `unsafe`-вызовы уровня 1; наружу — только
//! безопасный API. Владение ресурсами — RAII: модель, контекст и сэмплер
//! освобождаются в `Drop`.
//!
//! [derived] Ограничения v1: поштучный стриминг и grammar-constrained
//! tool-calls появятся вместе с `tensa-tools`/роутером; пока ответ чата
//! отдаётся целиком после генерации.

use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::Once;

use tensa_llama_sys as sys;

use crate::engine::{ChatMessage, FinishReason, LlmConfig, Role};
use crate::error::{LlmError, Result};
use crate::model::ModelInfo;
use crate::sampling::detect_stop;

static BACKEND_INIT: Once = Once::new();

/// Однократная инициализация бэкенда llama.cpp за процесс.
fn ensure_backend() {
    BACKEND_INIT.call_once(|| {
        // SAFETY: llama_backend_init вызывается ровно один раз (Once);
        // колбэк логов — статическая функция, живёт весь процесс.
        unsafe {
            sys::llama_backend_init();
            let callback: sys::ggml_log_callback = Some(log_callback);
            sys::llama_log_set(callback, ptr::null_mut());
        }
    });
}

/// Прокси-колбэк логов llama.cpp в `tracing` (иначе ggml пишет в stderr).
unsafe extern "C" fn log_callback(
    level: sys::ggml_log_level,
    text: *const std::os::raw::c_char,
    _user_data: *mut std::os::raw::c_void,
) {
    if text.is_null() {
        return;
    }
    // SAFETY: llama.cpp передаёт валидную C-строку; читаем без владения.
    let message = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    let message = message.trim_end();
    // llama.cpp иногда шлёт сообщения из одной пунктуации («.»): это не
    // ошибка и не должно спамить лог тысячами строк.
    if message.is_empty()
        || message
            .trim_matches(|c: char| !c.is_alphanumeric())
            .is_empty()
    {
        return;
    }
    if level >= sys::ggml_log_level_GGML_LOG_LEVEL_ERROR {
        tracing::error!(target: "llama.cpp", "{message}");
    } else if level >= sys::ggml_log_level_GGML_LOG_LEVEL_WARN {
        tracing::warn!(target: "llama.cpp", "{message}");
    } else {
        tracing::debug!(target: "llama.cpp", "{message}");
    }
}

/// Результат генерации нативного движка.
pub(crate) struct NativeOutput {
    pub text: String,
    pub tokens_prompt: u32,
    pub tokens_generated: u32,
    pub finish_reason: FinishReason,
}

/// Владелец ресурсов llama.cpp: модель, контекст, vocab.
pub(crate) struct NativeEngine {
    model: *mut sys::llama_model,
    ctx: *mut sys::llama_context,
    vocab: *const sys::llama_vocab,
    n_batch: u32,
    n_ctx: u32,
    chat_template: Option<String>,
}

// SAFETY: указатели llama.cpp никогда не используются конкурентно: снаружи
// объект живёт в `tokio::sync::Mutex`, а методы требуют `&mut self`.
unsafe impl Send for NativeEngine {}

impl NativeEngine {
    /// Загрузить GGUF-модель и создать контекст.
    pub(crate) fn load(config: &LlmConfig) -> Result<Self> {
        ensure_backend();
        let path = cstring(&config.model_path.to_string_lossy())?;

        // SAFETY: params — POD-структуры C-API; указатели остаются дефолтными
        // (никаких буферов, которые нужно держать живыми).
        unsafe {
            let mut model_params = sys::llama_model_default_params();
            model_params.n_gpu_layers = config.gpu_layers as i32;
            let model = sys::llama_model_load_from_file(path.as_ptr(), model_params);
            if model.is_null() {
                return Err(LlmError::ModelCorrupted(format!(
                    "не удалось загрузить {}",
                    config.model_path.display()
                )));
            }

            let vocab = sys::llama_model_get_vocab(model);
            if vocab.is_null() {
                sys::llama_model_free(model);
                return Err(LlmError::ModelCorrupted("в модели нет vocab".to_string()));
            }

            let mut ctx_params = sys::llama_context_default_params();
            ctx_params.n_ctx = config.context_size;
            ctx_params.n_batch = config.batch_size;
            if config.threads > 0 {
                ctx_params.n_threads = config.threads as i32;
                ctx_params.n_threads_batch = config.threads as i32;
            }

            let ctx = sys::llama_init_from_model(model, ctx_params);
            if ctx.is_null() {
                sys::llama_model_free(model);
                return Err(LlmError::Llama(
                    "llama_init_from_model вернул null".to_string(),
                ));
            }

            let chat_template = read_meta_str(model, "tokenizer.chat_template");

            Ok(Self {
                model,
                ctx,
                vocab,
                n_batch: config.batch_size,
                n_ctx: config.context_size,
                chat_template,
            })
        }
    }

    /// Информация о загруженной модели (файл, квантизация, контекст).
    pub(crate) fn model_info(&self, config: &LlmConfig) -> ModelInfo {
        let size_bytes = std::fs::metadata(&config.model_path)
            .map(|meta| meta.len())
            .unwrap_or(0);
        let quantization = self
            .file_type_name()
            .unwrap_or_else(|| "unknown".to_string());
        ModelInfo {
            path: config.model_path.clone(),
            tier: config.model_tier,
            size_bytes,
            quantization,
            context_size: self.n_ctx,
        }
    }

    /// Собрать промпт из сообщений по chat-шаблону модели.
    pub(crate) fn apply_chat_template(&self, messages: &[ChatMessage]) -> Result<String> {
        let Some(template) = self.chat_template.as_deref() else {
            // Модель без chat-шаблона — простая ролевая разметка. [derived]
            let mut prompt = String::new();
            for message in messages {
                let role = match message.role {
                    Role::System => "System",
                    Role::User => "User",
                    Role::Assistant => "Assistant",
                };
                prompt.push_str(role);
                prompt.push_str(": ");
                prompt.push_str(&message.content);
                prompt.push('\n');
            }
            prompt.push_str("Assistant:");
            return Ok(prompt);
        };

        let c_template = cstring(template)?;
        let prepared: Vec<(CString, CString)> = messages
            .iter()
            .map(|message| {
                let role = match message.role {
                    Role::System => "system",
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                Ok((cstring(role)?, cstring(&message.content)?))
            })
            .collect::<Result<Vec<_>>>()?;
        let chat: Vec<sys::llama_chat_message> = prepared
            .iter()
            .map(|(role, content)| sys::llama_chat_message {
                role: role.as_ptr(),
                content: content.as_ptr(),
            })
            .collect();

        // Первый вызов — необходимый размер буфера (buf = null, len = 0).
        // SAFETY: указатели получены из CString, живых до конца функции.
        let needed = unsafe {
            sys::llama_chat_apply_template(
                c_template.as_ptr(),
                chat.as_ptr(),
                chat.len(),
                true,
                ptr::null_mut(),
                0,
            )
        };
        if needed < 0 {
            return Err(LlmError::Llama(format!("chat template rc={needed}")));
        }

        let mut buffer = vec![0u8; needed as usize + 1];
        // SAFETY: буфер достаточного размера; chat/template живы.
        let written = unsafe {
            sys::llama_chat_apply_template(
                c_template.as_ptr(),
                chat.as_ptr(),
                chat.len(),
                true,
                buffer.as_mut_ptr() as *mut i8,
                buffer.len() as i32,
            )
        };
        if written < 0 {
            return Err(LlmError::Llama(format!("chat template rc={written}")));
        }
        buffer.truncate(written as usize);
        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }

    /// Сгенерировать ответ на промпт (блокирующий цикл llama.cpp).
    pub(crate) fn generate(
        &mut self,
        prompt: &str,
        max_tokens: u32,
        params: &crate::sampling::SamplingParams,
        stop: &[String],
    ) -> Result<NativeOutput> {
        let tokens = self.tokenize(prompt, true)?;
        if tokens.is_empty() {
            return Err(LlmError::Llama(
                "пустой промпт после токенизации".to_string(),
            ));
        }
        let tokens_prompt = tokens.len() as u32;
        let sampler = self.build_sampler(
            params.temperature,
            params.top_p,
            params.top_k,
            params.repeat_penalty,
        )?;
        let _sampler_guard = SamplerGuard(sampler);

        // SAFETY: эксклюзивный доступ (&mut self); память КV-кэша — C-API.
        unsafe {
            let memory = sys::llama_get_memory(self.ctx);
            if !memory.is_null() {
                sys::llama_memory_clear(memory, true);
            }
        }

        // Декодируем промпт чанками не больше n_batch.
        // (llama_batch_get_one ведёт позиции автоматически — см. llama.h.)
        let batch_size = self.n_batch.max(1) as usize;
        let mut offset = 0usize;
        while offset < tokens.len() {
            let end = (offset + batch_size).min(tokens.len());
            let mut chunk = tokens[offset..end].to_vec();
            // SAFETY: chunk живёт до конца вызова llama_decode.
            let batch = unsafe { sys::llama_batch_get_one(chunk.as_mut_ptr(), chunk.len() as i32) };
            let rc = unsafe { sys::llama_decode(self.ctx, batch) };
            if rc != 0 {
                return Err(LlmError::Llama(format!("llama_decode(prompt) rc={rc}")));
            }
            offset = end;
        }

        let mut text = String::new();
        let mut generated = 0u32;
        let mut finish_reason = FinishReason::Length;

        while generated < max_tokens {
            // SAFETY: сэмплер и контекст валидны; idx = -1 — логиты последнего
            // токена батча (канонический паттерн llama.cpp).
            let token = unsafe { sys::llama_sampler_sample(sampler, self.ctx, -1) };
            unsafe { sys::llama_sampler_accept(sampler, token) };

            // SAFETY: vocab принадлежит модели в этом же объекте.
            if unsafe { sys::llama_vocab_is_eog(self.vocab, token) } {
                finish_reason = FinishReason::Stop;
                break;
            }

            let piece = self.token_to_piece(token)?;
            text.push_str(&piece);
            generated += 1;

            if !stop.is_empty() && detect_stop(&text, stop).is_some() {
                finish_reason = FinishReason::Stop;
                break;
            }

            let mut next = vec![token];
            // SAFETY: next живёт до конца вызова llama_decode.
            let batch = unsafe { sys::llama_batch_get_one(next.as_mut_ptr(), 1) };
            let rc = unsafe { sys::llama_decode(self.ctx, batch) };
            if rc != 0 {
                finish_reason = FinishReason::Error;
                break;
            }
        }

        Ok(NativeOutput {
            text,
            tokens_prompt,
            tokens_generated: generated,
            finish_reason,
        })
    }

    /// Токенизировать текст (add_special — добавлять BOS и т. п.).
    fn tokenize(&self, text: &str, add_special: bool) -> Result<Vec<sys::llama_token>> {
        let c_text = cstring(text)?;
        let len = c_text.as_bytes().len() as i32;

        // Первый вызов — необходимый размер (buf = null, n = 0).
        // SAFETY: запрос размера буфера штатно поддерживается C-API.
        let rc = unsafe {
            sys::llama_tokenize(
                self.vocab,
                c_text.as_ptr(),
                len,
                ptr::null_mut(),
                0,
                add_special,
                true,
            )
        };
        if rc == 0 {
            return Ok(Vec::new());
        }
        let required = if rc < 0 { -rc } else { rc };

        let mut tokens = vec![0 as sys::llama_token; required as usize];
        // SAFETY: буфер достаточного размера; текст живёт до конца вызова.
        let written = unsafe {
            sys::llama_tokenize(
                self.vocab,
                c_text.as_ptr(),
                len,
                tokens.as_mut_ptr(),
                required,
                add_special,
                true,
            )
        };
        if written < 0 {
            return Err(LlmError::Llama(format!("llama_tokenize rc={written}")));
        }
        tokens.truncate(written as usize);
        Ok(tokens)
    }

    /// Преобразовать токен в текстовый кусок (UTF-8).
    fn token_to_piece(&self, token: sys::llama_token) -> Result<String> {
        let mut buffer = vec![0u8; 256];
        // SAFETY: буфер живёт на время вызова; lstrip = 0 (не срезать пробелы).
        let written = unsafe {
            sys::llama_token_to_piece(
                self.vocab,
                token,
                buffer.as_mut_ptr() as *mut i8,
                buffer.len() as i32,
                0,
                false,
            )
        };
        if written < 0 {
            // Буфер мал — нужен размер -written.
            let required = -written;
            let mut bigger = vec![0u8; required as usize];
            // SAFETY: буфер увеличен до требуемого размера.
            let written = unsafe {
                sys::llama_token_to_piece(
                    self.vocab,
                    token,
                    bigger.as_mut_ptr() as *mut i8,
                    bigger.len() as i32,
                    0,
                    false,
                )
            };
            if written < 0 {
                return Err(LlmError::Llama(format!("token_to_piece rc={written}")));
            }
            bigger.truncate(written as usize);
            return Ok(String::from_utf8_lossy(&bigger).into_owned());
        }
        buffer.truncate(written as usize);
        Ok(String::from_utf8_lossy(&buffer).into_owned())
    }

    /// Собрать цепочку сэмплеров (penalties → top_k → top_p → temp → dist).
    fn build_sampler(
        &self,
        temperature: f32,
        top_p: f32,
        top_k: u32,
        repeat_penalty: f32,
    ) -> Result<*mut sys::llama_sampler> {
        // SAFETY: init-функции создают владельцев; цепочка получает владение
        // через chain_add и освободит их в llama_sampler_free.
        unsafe {
            let chain = sys::llama_sampler_chain_init(sys::llama_sampler_chain_default_params());
            if chain.is_null() {
                return Err(LlmError::Llama("sampler chain init".to_string()));
            }

            if (repeat_penalty - 1.0).abs() > f32::EPSILON {
                let vocab_size = sys::llama_vocab_n_tokens(self.vocab);
                let penalties =
                    sys::llama_sampler_init_penalties(vocab_size, 64, repeat_penalty, 0.0, 0.0);
                if penalties.is_null() {
                    sys::llama_sampler_free(chain);
                    return Err(LlmError::Llama("sampler penalties init".to_string()));
                }
                sys::llama_sampler_chain_add(chain, penalties);
            }

            if temperature <= 0.0 {
                let greedy = sys::llama_sampler_init_greedy();
                if greedy.is_null() {
                    sys::llama_sampler_free(chain);
                    return Err(LlmError::Llama("sampler greedy init".to_string()));
                }
                sys::llama_sampler_chain_add(chain, greedy);
            } else {
                sys::llama_sampler_chain_add(chain, sys::llama_sampler_init_top_k(top_k as i32));
                sys::llama_sampler_chain_add(chain, sys::llama_sampler_init_top_p(top_p, 1));
                sys::llama_sampler_chain_add(chain, sys::llama_sampler_init_temp(temperature));
                sys::llama_sampler_chain_add(chain, sys::llama_sampler_init_dist(u32::MAX));
            }

            Ok(chain)
        }
    }

    /// Имя квантизации из метаданных (`general.file_type`).
    fn file_type_name(&self) -> Option<String> {
        let raw = read_meta_str(self.model, "general.file_type")?;
        let code: u32 = raw.trim().parse().ok()?;
        Some(ftype_name(code))
    }
}

impl Drop for NativeEngine {
    fn drop(&mut self) {
        // SAFETY: указатели принадлежат объекту; порядок — контекст, модель.
        unsafe {
            if !self.ctx.is_null() {
                sys::llama_free(self.ctx);
            }
            if !self.model.is_null() {
                sys::llama_model_free(self.model);
            }
        }
    }
}

/// RAII-обёртка сэмплера (освобождает цепочку).
struct SamplerGuard(*mut sys::llama_sampler);

impl Drop for SamplerGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: владелец цепочки — этот guard, освобождаем один раз.
            unsafe { sys::llama_sampler_free(self.0) };
        }
    }
}

/// CString с понятной ошибкой.
fn cstring(value: &str) -> Result<CString> {
    CString::new(value).map_err(|_| LlmError::Config("строка содержит NUL".to_string()))
}

/// Максимальный размер значения метаданных (защита от патологического роста).
const META_VALUE_LIMIT: usize = 1 << 20;

/// Результат одного чтения строкового значения метаданных.
#[derive(Debug, PartialEq)]
enum MetaRead {
    /// Ключа нет (или ошибка чтения).
    Missing,
    /// Значение усечено буфером — нужен буфер указанного размера.
    NeedMore(usize),
    /// Готовое значение.
    Value(String),
}

/// Разобрать результат `llama_model_meta_val_str`.
///
/// Семантика `snprintf`: возвращается полная длина значения без NUL; если
/// она не меньше размера буфера — значение усечено и его нужно перечитать
/// буфером большего размера.
fn meta_value_chunk(written: i32, buffer: Vec<u8>) -> MetaRead {
    if written < 0 {
        return MetaRead::Missing;
    }
    let written = written as usize;
    if written >= buffer.len() {
        return MetaRead::NeedMore(written + 1);
    }
    let mut bytes = buffer;
    bytes.truncate(written);
    // Страховка: не тащить NUL-паддинг в String.
    while bytes.last() == Some(&0) {
        bytes.pop();
    }
    MetaRead::Value(String::from_utf8_lossy(&bytes).into_owned())
}

/// Прочитать строку метаданных модели (`meta_val_str`) с динамическим
/// буфером: первый запрос — короткий, при усечении буфер растёт до полной
/// длины значения (например, chat-шаблон Gemma 4 — ~17 КиБ).
fn read_meta_str(model: *const sys::llama_model, key: &str) -> Option<String> {
    let c_key = CString::new(key).ok()?;
    let mut size = 4096usize;
    loop {
        let mut buffer = vec![0u8; size];
        // SAFETY: ключ и буфер живут на время вызова.
        let written = unsafe {
            sys::llama_model_meta_val_str(
                model,
                c_key.as_ptr(),
                buffer.as_mut_ptr() as *mut i8,
                buffer.len(),
            )
        };
        match meta_value_chunk(written, buffer) {
            MetaRead::Missing => return None,
            MetaRead::Value(value) => return Some(value),
            MetaRead::NeedMore(needed) => {
                if needed > META_VALUE_LIMIT {
                    return None;
                }
                size = needed;
            }
        }
    }
}

/// Имя квантизации по коду `llama_ftype` (константы llama.cpp v0.7.0).
fn ftype_name(code: u32) -> String {
    let table: &[(u32, &str)] = &[
        (sys::llama_ftype_LLAMA_FTYPE_ALL_F32 as u32, "F32"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_F16 as u32, "F16"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_0 as u32, "Q4_0"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_1 as u32, "Q4_1"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_0 as u32, "Q5_0"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_1 as u32, "Q5_1"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q8_0 as u32, "Q8_0"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q2_K as u32, "Q2_K"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q2_K_S as u32, "Q2_K_S"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q3_K_S as u32, "Q3_K_S"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q3_K_M as u32, "Q3_K_M"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q3_K_L as u32, "Q3_K_L"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_K_S as u32, "Q4_K_S"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q4_K_M as u32, "Q4_K_M"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_K_S as u32, "Q5_K_S"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q5_K_M as u32, "Q5_K_M"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_Q6_K as u32, "Q6_K"),
        (sys::llama_ftype_LLAMA_FTYPE_MOSTLY_IQ4_NL as u32, "IQ4_NL"),
    ];
    table
        .iter()
        .find(|(value, _)| *value == code)
        .map(|(_, name)| (*name).to_string())
        .unwrap_or_else(|| format!("ftype:{code}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ftype_table_matches_spec() {
        assert_eq!(ftype_name(11), "Q3_K_S");
        assert_eq!(ftype_name(14), "Q4_K_S");
        assert_eq!(ftype_name(15), "Q4_K_M");
        assert_eq!(ftype_name(9999), "ftype:9999");
    }

    #[test]
    fn cstring_rejects_nul() {
        assert!(cstring("ок\nстрока").is_ok());
        assert!(cstring("nul\0внутри").is_err());
    }
}

#[test]
fn meta_value_chunk_reads_normal_value() {
    let mut buffer = vec![0u8; 32];
    let value = "Q3_K_S";
    buffer[..value.len()].copy_from_slice(value.as_bytes());
    assert_eq!(
        meta_value_chunk(value.len() as i32, buffer),
        MetaRead::Value(value.to_string())
    );
}

#[test]
fn meta_value_chunk_requests_bigger_buffer_for_large_value() {
    let buffer = vec![0u8; 8192];
    assert_eq!(meta_value_chunk(17_336, buffer), MetaRead::NeedMore(17_337));
}

#[test]
fn meta_value_chunk_missing_on_error() {
    assert_eq!(meta_value_chunk(-1, vec![0u8; 8]), MetaRead::Missing);
}

#[test]
fn meta_value_chunk_trims_trailing_nul_padding() {
    let mut buffer = vec![0u8; 12];
    buffer[..5].copy_from_slice(b"value");
    assert_eq!(
        meta_value_chunk(8, buffer),
        MetaRead::Value("value".to_string())
    );
}
