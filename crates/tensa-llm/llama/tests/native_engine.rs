//! Нативные integration-тесты tensa-llama (фича `native`, TASK-ENV-001).
//!
//! Требуют скачанную GGUF-модель (~1.6 ГБ):
//! путь — `TENSA_TEST_MODEL` или `~/.tensa/models/gemma-4-E2B-it-XS-TS-Q3_K_S.gguf`
//! (см. README `tensa-llama-sys`).
//!
//! Запуск (по умолчанию тесты игнорируются — инференс долгий):
//! ```text
//! cargo test -p tensa-llama --features native -- --ignored --nocapture
//! ```
#![cfg(feature = "native")]

use std::path::PathBuf;

use tensa_llama::*;

/// Путь к тестовой модели (env `TENSA_TEST_MODEL` или каталог Tensa).
fn model_path() -> Option<PathBuf> {
    if let Ok(path) = std::env::var("TENSA_TEST_MODEL") {
        let path = PathBuf::from(path);
        return path.exists().then_some(path);
    }
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .ok()?;
    let path = PathBuf::from(home)
        .join(".tensa")
        .join("models")
        .join("gemma-4-E2B-it-XS-TS-Q3_K_S.gguf");
    path.exists().then_some(path)
}

fn config_for(path: PathBuf) -> LlmConfig {
    LlmConfig {
        model_path: path,
        context_size: 2048,
        max_tokens: 32,
        temperature: 0.0, // greedy — детерминированный результат
        ..LlmConfig::default()
    }
}

#[tokio::test]
#[ignore = "требует скачанную GGUF ~1.6 ГБ; запускать с --ignored"]
async fn loads_real_model_and_generates() {
    let Some(path) = model_path() else {
        panic!(
            "модель не найдена: задайте TENSA_TEST_MODEL или скачайте GGUF в ~/.tensa/models/"
        );
    };

    let engine = LlamaEngine::new();
    LlamaEngine::load(&engine, config_for(path))
        .await
        .expect("загрузка модели");

    let health = LlmEngine::health(&engine).await.expect("health");
    assert!(health.loaded);
    assert!(health.native_engine);
    assert_eq!(health.context_size, 2048);

    let info = LlamaEngine::model_info(&engine).expect("model_info");
    assert!(info.size_bytes > 0);
    assert_eq!(info.tier, ModelTier::Xs);

    let response = LlmEngine::generate(
        &engine,
        GenerateRequest {
            prompt: "Назови одним словом приветствие:".to_string(),
            system_prompt: None,
            max_tokens: Some(16),
            temperature: Some(0.0),
            stop: vec![],
        },
    )
    .await
    .expect("генерация");
    println!(
        "квантизация: {} | ответ: {:?} ({} токенов, {} мс)",
        info.quantization, response.text, response.tokens_generated, response.time_ms
    );
    assert!(
        !response.text.trim().is_empty(),
        "пустой ответ модели: {response:?}"
    );
    assert!(response.tokens_prompt > 0);

    LlmEngine::unload(&engine).await.expect("unload");
    let health = LlmEngine::health(&engine).await.expect("health после unload");
    assert!(!health.loaded);
    assert!(LlmEngine::model_info(&engine).is_none());
}

#[tokio::test]
#[ignore = "требует скачанную GGUF ~1.6 ГБ; запускать с --ignored"]
async fn chat_with_template_generates_reply() {
    let Some(path) = model_path() else {
        panic!(
            "модель не найдена: задайте TENSA_TEST_MODEL или скачайте GGUF в ~/.tensa/models/"
        );
    };

    let engine = LlamaEngine::new();
    LlamaEngine::load(&engine, config_for(path))
        .await
        .expect("загрузка модели");

    let response = LlmEngine::chat(
        &engine,
        ChatRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "Скажи «привет» одним словом.".to_string(),
            }],
            tools: vec![],
            max_tokens: Some(16),
            temperature: Some(0.0),
        },
    )
    .await
    .expect("чат");

    println!("ответ чата: {:?}", response.message.content);
    assert!(matches!(response.message.role, Role::Assistant));
    assert!(
        !response.message.content.trim().is_empty(),
        "пустой ответ чата: {response:?}"
    );
    assert!(response.tool_calls.is_empty());

    // Потоковая форма отдаёт текст и завершается пустым куском.
    let mut stream = LlmEngine::chat_stream(
        &engine,
        ChatRequest {
            messages: vec![ChatMessage {
                role: Role::User,
                content: "Привет!".to_string(),
            }],
            tools: vec![],
            max_tokens: Some(8),
            temperature: Some(0.0),
        },
    )
    .await
    .expect("поток");

    use futures::StreamExt;
    let mut pieces = Vec::new();
    while let Some(item) = stream.next().await {
        pieces.push(item.expect("кусок потока"));
    }
    assert!(!pieces.is_empty());
    assert_eq!(pieces.last().map(String::as_str), Some(""));
}
