//! Integration-тесты tensa-llama: жизненный цикл движка без нативного слоя
//! (ТЗ tensa-llm: ошибки «модель не найдена», «движок недоступен»,
//! «не загружено», health/model_info).

use std::path::PathBuf;
use tensa_llama::*;

fn engine() -> LlamaEngine {
    LlamaEngine::new()
}

fn config_with_path(path: PathBuf) -> LlmConfig {
    LlmConfig {
        model_path: path,
        ..LlmConfig::default()
    }
}

// --- Конфигурация ---

#[test]
fn default_config_matches_tz() {
    let c = LlmConfig::default();
    assert_eq!(c.context_size, 4096);
    assert_eq!(c.threads, 0);
    assert_eq!(c.batch_size, 512);
    assert_eq!(c.gpu_layers, 0);
    assert_eq!(c.temperature, 0.7);
    assert_eq!(c.top_p, 0.95);
    assert_eq!(c.top_k, 40);
    assert_eq!(c.max_tokens, 2048);
    assert_eq!(c.repeat_penalty, 1.1);
    assert_eq!(c.model_tier, ModelTier::Xs);
}

#[test]
fn config_validation_errors() {
    // Пустой путь.
    assert!(LlmConfig::default().validate().is_err());
    // Нулевой контекст.
    let mut c = config_with_path(PathBuf::from("model.gguf"));
    c.context_size = 0;
    assert!(c.validate().is_err());
    // Плохой batch.
    let mut c = config_with_path(PathBuf::from("model.gguf"));
    c.batch_size = 0;
    assert!(c.validate().is_err());
    // Нулевые max_tokens.
    let mut c = config_with_path(PathBuf::from("model.gguf"));
    c.max_tokens = 0;
    assert!(c.validate().is_err());
    // Плохой sampling (проверяется через конфиг).
    let mut c = config_with_path(PathBuf::from("model.gguf"));
    c.top_p = 2.0;
    assert!(c.validate().is_err());
}

// --- Жизненный цикл ---

#[tokio::test]
async fn load_missing_model_is_not_found() {
    let e = engine();
    let cfg = config_with_path(PathBuf::from("C:/no/such/model-XS.gguf"));
    let err = LlmEngine::load(&e, cfg).await.expect_err("missing");
    assert!(matches!(err, LlmError::ModelNotFound(_)), "got: {err:?}");
}

#[tokio::test]
async fn load_existing_file_reports_engine_unavailable() {
    let e = engine();
    // Реальный файл есть (пустой) — следующий барьер: нативный слой.
    let path = std::env::temp_dir().join(format!("tensa_llm_{}.gguf", std::process::id()));
    std::fs::write(&path, b"not a real gguf").expect("write temp");
    let cfg = config_with_path(path.clone());
    let err = LlmEngine::load(&e, cfg).await.expect_err("unavailable");
    match &err {
        LlmError::EngineUnavailable(msg) => {
            assert!(msg.contains("ADR-003"), "msg: {msg}");
        }
        other => panic!("expected EngineUnavailable, got {other:?}"),
    }
    std::fs::remove_file(&path).ok();

    // После неудачной загрузки генерция недоступна.
    let err = LlmEngine::generate(
        &e,
        GenerateRequest {
            prompt: "привет".into(),
            system_prompt: None,
            max_tokens: None,
            temperature: None,
            stop: vec![],
        },
    )
    .await
    .expect_err("not loaded");
    assert!(matches!(err, LlmError::NotLoaded));
}

#[tokio::test]
async fn generate_chat_stream_before_load_not_loaded() {
    let e = engine();

    let err = LlmEngine::generate(
        &e,
        GenerateRequest {
            prompt: "x".into(),
            system_prompt: None,
            max_tokens: Some(16),
            temperature: Some(0.0),
            stop: vec![],
        },
    )
    .await
    .expect_err("generate");
    assert!(matches!(err, LlmError::NotLoaded));

    let err = LlmEngine::chat(
        &e,
        ChatRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "привет".into(),
            }],
            tools: vec![],
            max_tokens: None,
            temperature: None,
        },
    )
    .await
    .expect_err("chat");
    assert!(matches!(err, LlmError::NotLoaded));

    let err = match LlmEngine::chat_stream(
        &e,
        ChatRequest {
            messages: vec![],
            tools: vec![],
            max_tokens: None,
            temperature: None,
        },
    )
    .await
    {
        Ok(_) => panic!("expected NotLoaded, got stream"),
        Err(err) => err,
    };
    assert!(matches!(err, LlmError::NotLoaded));
}

#[tokio::test]
async fn unload_and_health_lifecycle() {
    let e = engine();

    let h = LlmEngine::health(&e).await.expect("health");
    assert!(!h.loaded);
    assert!(!h.native_engine, "нативный слой ещё не подключён");
    assert_eq!(h.context_size, 0);
    assert!(h.model.is_none());

    // unload без загрузки — идемпотентен.
    LlmEngine::unload(&e).await.expect("unload");
    assert!(LlmEngine::model_info(&e).is_none());
}

#[tokio::test]
async fn tool_definition_and_call_shapes() {
    // Формы типов tool use из ТЗ (JSON-схема параметров).
    let def = ToolDefinition {
        name: "send_email".into(),
        description: "Отправить письмо".into(),
        parameters: serde_json::json!({
            "type": "object",
            "properties": { "to": { "type": "string" } },
            "required": ["to"]
        }),
    };
    let json = serde_json::to_value(&def).expect("ser");
    assert_eq!(json["name"], "send_email");
    assert_eq!(json["parameters"]["type"], "object");

    let call = ToolCall {
        name: "send_email".into(),
        params: serde_json::json!({ "to": "a@b.c" }),
    };
    let roundtrip: ToolCall =
        serde_json::from_value(serde_json::to_value(&call).expect("ser")).expect("de");
    assert_eq!(roundtrip.name, "send_email");
}

#[test]
fn stop_check_helper() {
    let e = engine();
    let stop = vec!["</s>".to_string()];
    // «текст» — 5 кириллических символов по 2 байта = offset 10.
    assert_eq!(e.check_stop("текст</s>", &stop), Some((10, 4)));
    assert_eq!(e.check_stop("без стопа", &stop), None);
}

#[test]
fn native_engine_flag_is_false_until_ffi() {
    // Константы отражают состояние уровня 1 (tensa-llama-sys);
    // black_box — чтобы проверка была рантайм-проверкой (лint clippy).
    let native = std::hint::black_box(NATIVE_ENGINE);
    let ready = std::hint::black_box(tensa_llama_sys::NATIVE_READY);
    assert!(!native, "нативный движок ещё не скомпилирован");
    assert!(!ready, "уровень 1 (llama-sys) ещё не подключён");
}
