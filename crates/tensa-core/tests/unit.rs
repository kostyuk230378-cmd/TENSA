//! Unit-тесты tensa-core: Config, paths, first_run, init (по списку ТЗ).

use tensa_core::*;

fn test_node_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

// --- Config (Контракт 4) ---

#[test]
fn config_toml_roundtrip() {
    let node = test_node_id();
    let config = Config::default_with_node(&node);
    let path = std::env::temp_dir().join(format!("tensa_cfg_{}.toml", uuid::Uuid::new_v4()));
    config.save(&path).expect("save");
    let loaded = Config::load(&path).expect("load");
    assert_eq!(loaded.general.node_id, node);
    assert_eq!(loaded.general.language, Language::Ru);
    std::fs::remove_file(&path).ok();
}

#[test]
fn config_validation() {
    let mut config = Config::default_with_node(&test_node_id());
    config.validate().expect("default is valid");
    // Пустой node_id невалиден.
    config.general.node_id = String::new();
    assert!(config.validate().is_err());
    // Не-UUID node_id невалиден.
    config.general.node_id = "not-a-uuid".into();
    assert!(config.validate().is_err());
}

#[test]
fn config_missing_file_error() {
    let path = std::env::temp_dir().join(format!("no_such_{}.toml", uuid::Uuid::new_v4()));
    assert!(Config::load(&path).is_err());
}

#[test]
fn secrets_not_in_config_serialization() {
    // Контракт 4: секреты только в secrets.toml. В Config нет полей токенов.
    let config = Config::default_with_node(&test_node_id());
    let toml_text = toml::to_string(&config).expect("toml");
    assert!(!toml_text.contains("bot_token"));
    assert!(!toml_text.contains("api_key"));
}

// --- Paths ---

#[test]
fn paths_layout() {
    let paths = TensaPaths::with_root(std::env::temp_dir().join("tensa_paths_test"));
    assert!(paths.config_file().ends_with("config.toml"));
    assert!(paths.secrets_file().ends_with("secrets.toml"));
    assert!(paths.approval_db().ends_with("approval.db"));
    assert!(paths.notes_db().ends_with("notes.db"));
    paths.ensure_dirs().expect("dirs");
    assert!(paths.memory_dir().exists());
    assert!(paths.models_dir().exists());
}

// --- First-run wizard (dop_tz_core) ---

#[tokio::test]
async fn wizard_incomplete_rejected() {
    let answers = WizardAnswers::default();
    assert!(!answers.complete());

    let root = std::env::temp_dir().join(format!("tensa_wizard_{}", uuid::Uuid::new_v4()));
    let wizard = FirstRunWizard::new(
        root.join("config.toml"),
        root.join("secrets.toml"),
        None,
        test_node_id(),
    )
    .await
    .expect("wizard");
    assert!(wizard.run().await.is_err());
    let _ = std::fs::remove_dir_all(root);
}

#[tokio::test]
async fn wizard_complete_flow() {
    let root = std::env::temp_dir().join(format!("tensa_wizard_{}", uuid::Uuid::new_v4()));
    let wizard = FirstRunWizard::new(
        root.join("config.toml"),
        root.join("secrets.toml"),
        None,
        test_node_id(),
    )
    .await
    .expect("wizard");
    wizard.step_language(Language::Ru).await.expect("язык");
    wizard
        .step_introduction(
            "Александр".into(),
            "Шеф".into(),
            "Tensa".into(),
            "Europe/Moscow".into(),
        )
        .await
        .expect("знакомство");

    let answers = wizard.answers().await;
    assert!(answers.complete());
    assert_eq!(answers.owner_name.as_deref(), Some("Александр"));
    assert_eq!(answers.owner_title.as_deref(), Some("Шеф"));

    let result = wizard.step_finish().await.expect("finish");
    assert!(!result.core_memory_initialized);
    assert_eq!(result.config.general.language, Language::Ru);
    let _ = std::fs::remove_dir_all(root);
}

// --- Инициализация памяти (Контракт 13) ---

#[test]
fn init_default_blocks() {
    let params = InitParams::default();
    validate_params(&params).expect("valid");
    let blocks = default_blocks(&params);
    let names: Vec<&str> = blocks.iter().map(|(b, _)| b.as_str()).collect();
    // Контракт 13: ровно 4 блока.
    assert_eq!(names, vec!["persona", "human", "tasks", "preferences"]);
    // human содержит имя владельца и обращение.
    let human = &blocks[1].1;
    assert!(human.contains("Владелец"));
    assert!(human.contains("Шеф"));
    // persona — базовая идентичность Tensa.
    assert!(blocks[0].1.contains("Tensa"));
}

#[test]
fn init_params_custom_owner() {
    let params = InitParams {
        owner_name: "Иван".into(),
        owner_title: "Босс".into(),
        secretary_name: "Tensa".into(),
        language: Language::En,
        timezone: "Asia/Almaty".into(),
    };
    let blocks = default_blocks(&params);
    let human = &blocks[1].1;
    assert!(human.contains("Иван"));
    assert!(human.contains("Босс"));
    assert!(human.contains("Asia/Almaty"));
}

#[test]
fn init_params_validation() {
    let params = InitParams {
        owner_name: String::new(),
        ..InitParams::default()
    };
    assert!(validate_params(&params).is_err());
    let params = InitParams {
        owner_name: "X".into(),
        timezone: String::new(),
        ..InitParams::default()
    };
    assert!(validate_params(&params).is_err());
}
