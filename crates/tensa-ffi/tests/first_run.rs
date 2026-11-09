//! Интеграционные тесты FFI-мастера первого запуска (Контракт 9, доп. ТЗ
//! tensa-core): полный цикл с записью config/secrets/core memory, ошибки
//! обязательных шагов и честная заглушка шага почты.

use std::path::PathBuf;

use tensa_ffi::FirstRunWizard;
use tensa_memory::{CoreMemoryConfig, CoreMemoryStore as _, SqliteCoreMemoryStore};

/// Временный корень данных.
fn temp_root() -> PathBuf {
    let root = std::env::temp_dir().join(format!("tensa-ffi-wizard-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&root).expect("временный корень");
    root
}

#[tokio::test]
async fn full_flow_writes_config_secrets_and_memory() {
    let root = temp_root();
    let config_path = root.join("config.toml");
    let wizard = FirstRunWizard::new(config_path.to_string_lossy().to_string())
        .await
        .expect("wizard");
    assert!(
        wizard.is_first_run().await.expect("is_first_run"),
        "без config.toml это первый запуск"
    );

    wizard.step_language("ru".into()).await.expect("язык");
    wizard
        .step_introduction(
            "Иван".into(),
            "Шеф".into(),
            "Tensa".into(),
            "Europe/Moscow".into(),
        )
        .await
        .expect("знакомство");
    wizard
        .step_telegram("bot-secret".into(), "42".into())
        .await
        .expect("telegram");
    wizard.step_models().await.expect("модели");
    wizard.step_whatsapp(None).await.expect("whatsapp-пропуск");
    wizard.step_tailscale().await.expect("tailscale");

    let result = wizard.step_finish().await.expect("финиш");
    assert!(result.config_saved, "config.toml записан");
    assert!(result.secrets_saved, "secrets.toml записан");
    assert!(
        result.core_memory_initialized,
        "core memory инициализирована"
    );
    assert!(
        !wizard
            .is_first_run()
            .await
            .expect("is_first_run после финиша"),
        "config.toml существует — первый запуск завершён"
    );

    // config.toml: язык и telegram; secrets.toml: токен бота.
    let config = tensa_core::Config::load(&config_path).expect("config");
    assert_eq!(config.general.language, tensa_core::Language::Ru);
    let secrets = tensa_core::Secrets::load(&root.join("secrets.toml")).expect("secrets");
    assert_eq!(secrets.telegram.credential.as_deref(), Some("bot-secret"));

    // core memory: четыре блока в memory.db.
    let store = SqliteCoreMemoryStore::new(
        &root.join("memory.db"),
        CoreMemoryConfig::default(),
        "check".into(),
    )
    .await
    .expect("хранилище памяти");
    assert_eq!(store.get_all_blocks().await.expect("блоки").len(), 4);
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn finish_without_mandatory_steps_is_error() {
    let root = temp_root();
    let wizard = FirstRunWizard::new(root.join("config.toml").to_string_lossy().to_string())
        .await
        .expect("wizard");
    assert!(
        wizard.step_finish().await.is_err(),
        "финиш без языка и знакомства невозможен"
    );
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn unknown_language_is_error() {
    let root = temp_root();
    let wizard = FirstRunWizard::new(root.join("config.toml").to_string_lossy().to_string())
        .await
        .expect("wizard");
    assert!(wizard.step_language("de".into()).await.is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn step_email_saves_account_and_credential() {
    let root = temp_root();
    let config_path = root.join("config.toml");
    let wizard = FirstRunWizard::new(config_path.to_string_lossy().to_string())
        .await
        .expect("wizard");
    wizard.step_language("ru".into()).await.expect("язык");
    wizard
        .step_introduction(
            "Иван".into(),
            "Шеф".into(),
            "Tensa".into(),
            "Europe/Moscow".into(),
        )
        .await
        .expect("знакомство");
    wizard
        .step_email(tensa_ffi::EmailAccount {
            id: "personal".into(),
            label: "Личный".into(),
            email: Some("test@example.com".into()),
            credential: Some("app-password".into()),
            imap_host: "imap.example.com".into(),
            imap_port: 993,
            smtp_host: "smtp.example.com".into(),
            smtp_port: 587,
        })
        .await
        .expect("шаг почты");
    let result = wizard.step_finish().await.expect("финиш");
    assert!(result.config_saved && result.secrets_saved);

    // Аккаунт — в config, секрет — в secrets.toml.
    let config = tensa_core::Config::load(&config_path).expect("config");
    let accounts = config
        .email
        .get("accounts")
        .and_then(|value| value.get("personal"))
        .expect("аккаунт personal в config");
    assert_eq!(
        accounts.get("email").and_then(|value| value.as_str()),
        Some("test@example.com")
    );
    let secrets = tensa_core::Secrets::load(&root.join("secrets.toml")).expect("secrets");
    assert_eq!(
        secrets
            .email
            .accounts
            .get("personal")
            .and_then(|secret| secret.credential.clone()),
        Some("app-password".into())
    );
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn step_email_without_address_is_error() {
    // Ядро не принимает аккаунт без адреса (валидация step_email).
    let root = temp_root();
    let wizard = FirstRunWizard::new(root.join("config.toml").to_string_lossy().to_string())
        .await
        .expect("wizard");
    assert!(
        wizard
            .step_email(tensa_ffi::EmailAccount {
                id: "personal".into(),
                label: "Личный".into(),
                email: None,
                credential: None,
                imap_host: "imap.example.com".into(),
                imap_port: 993,
                smtp_host: "smtp.example.com".into(),
                smtp_port: 587,
            })
            .await
            .is_err(),
        "аккаунт без адреса отклоняется"
    );
    let _ = std::fs::remove_dir_all(root);
}
