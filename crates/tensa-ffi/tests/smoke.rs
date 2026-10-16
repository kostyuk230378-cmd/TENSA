//! Smoke-тесты FFI-обёртки: конструкторы и базовые вызовы на временном корне.
//!
//! Проверяются реальные интерфейсы (ApprovalStore, NotesStore) и классы
//! ошибок; заглушки проверяются на честный отказ.

use std::path::PathBuf;

use tensa_ffi::{ApprovalStore, NewNote, NotesStore, TensaError};

/// Временный корень данных для теста.
fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!("tensa-ffi-test-{}", uuid::Uuid::new_v4()))
}

#[tokio::test]
async fn approval_store_starts_empty() {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("каталог");
    let config = root.join("config.toml").to_string_lossy().to_string();

    let store = ApprovalStore::new(config).await.expect("конструктор");
    assert!(store.get_pending().await.expect("pending").is_empty());
    assert!(store
        .get_by_id(uuid::Uuid::new_v4().to_string())
        .await
        .expect("по id")
        .is_none());

    // Неверный id — понятная ошибка (внутренняя, текст в логе).
    assert!(store.get_by_id("not-a-uuid".to_string()).await.is_err());
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn notes_store_roundtrip() {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("каталог");
    let config = root.join("config.toml").to_string_lossy().to_string();

    let store = NotesStore::new(config).await.expect("конструктор");
    let created = store
        .create_note(NewNote {
            title: "FFI note".to_string(),
            body: "текст".to_string(),
            tags: vec!["ffi".to_string()],
        })
        .await
        .expect("создание");
    assert_eq!(created.title, "FFI note");

    let loaded = store
        .get_note(created.id.clone())
        .await
        .expect("чтение")
        .expect("заметка есть");
    assert_eq!(loaded.tags, vec!["ffi".to_string()]);

    let found = store
        .search_notes("FFI".to_string(), 10)
        .await
        .expect("поиск");
    assert_eq!(found.len(), 1);

    store
        .delete_note(created.id.clone())
        .await
        .expect("удаление");
    assert!(store
        .get_note(created.id)
        .await
        .expect("после удаления")
        .is_none());
    std::fs::remove_dir_all(root).ok();
}

#[tokio::test]
async fn stubs_answer_not_implemented() {
    let root = temp_root();
    std::fs::create_dir_all(&root).expect("каталог");
    let config = root.join("config.toml").to_string_lossy().to_string();

    let launcher = tensa_ffi::AppLauncher::new(config.clone())
        .await
        .expect("конструктор");
    assert!(!launcher.supports("браузер".to_string()));
    assert!(launcher.launch("браузер".to_string()).await.is_err());

    let voice = tensa_ffi::VoiceEngine::new(config)
        .await
        .expect("конструктор");
    assert!(voice
        .transcribe(vec![1, 2, 3], "wav".to_string())
        .await
        .is_err());
    std::fs::remove_dir_all(root).ok();
}

#[test]
fn error_classes_display() {
    assert_eq!(TensaError::ConfigError {}.to_string(), "config error");
    assert_eq!(TensaError::InternalError {}.to_string(), "internal error");
}
