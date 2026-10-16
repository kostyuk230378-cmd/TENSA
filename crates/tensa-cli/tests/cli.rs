//! Интеграционные тесты бинаря `tensa` (ТЗ tensa-cli: «Integration: все команды»).
//!
//! Каждый тест работает в собственном временном корне (`-c <root>/config.toml`);
//! реальный `~/.tensa` не затрагивается.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Путь к собранному бинарю `tensa` (cargo задаёт переменную для bin-таргета).
fn tensa_bin() -> &'static str {
    env!("CARGO_BIN_EXE_tensa")
}

/// Уникальный временный корень теста.
fn temp_root() -> PathBuf {
    std::env::temp_dir().join(format!("tensa-cli-test-{}", uuid::Uuid::new_v4()))
}

/// Запустить `tensa` с явным конфигом.
fn run(config: &Path, args: &[&str]) -> Output {
    Command::new(tensa_bin())
        .arg("-c")
        .arg(config)
        .args(args)
        .output()
        .expect("запуск бинаря tensa")
}

/// Инициализировать ядро в корне; вернуть путь к config.toml.
fn init_root(root: &Path) -> PathBuf {
    let config = root.join("config.toml");
    let output = run(
        &config,
        &[
            "init",
            "--owner-name",
            "Тест",
            "--secretary-name",
            "Tensa",
            "--timezone",
            "UTC",
        ],
    );
    assert!(
        output.status.success(),
        "init упал: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    config
}

/// stdout как текст.
fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// stderr как текст.
fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

#[test]
fn version_prints_name_and_json() {
    let root = temp_root();
    let config = root.join("config.toml");

    let output = run(&config, &["version"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("tensa"));

    let output = run(&config, &["--json", "version"]);
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON version");
    assert_eq!(json["name"], "tensa");
    assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
}

#[test]
fn help_lists_commands() {
    let root = temp_root();
    let output = run(&root.join("config.toml"), &["--help"]);
    assert!(output.status.success());
    let text = stdout(&output);
    for command in ["init", "benchmark", "approval", "notes", "nodes", "version"] {
        assert!(text.contains(command), "нет команды {command} в справке");
    }
}

#[test]
fn status_without_config_reports_error() {
    let root = temp_root();
    let output = run(&root.join("config.toml"), &["status"]);
    assert!(!output.status.success(), "ожидалась ошибка без конфига");
    assert!(
        stderr(&output).contains("конфиг не найден"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn init_creates_config_and_secrets_and_is_idempotent() {
    let root = temp_root();
    let config = init_root(&root);
    assert!(config.is_file(), "config.toml создан");
    assert!(root.join("secrets.toml").is_file(), "secrets.toml создан");

    // Конфиг валиден и разбирается ядром.
    let loaded = tensa_core::Config::load(&config).expect("конфиг читается");
    assert!(!loaded.general.node_id.is_empty());

    // Повторный init — не ошибка, лишь предупреждение.
    let output = run(
        &config,
        &["init", "--owner-name", "Тест", "--secretary-name", "Tensa"],
    );
    assert!(output.status.success());
    assert!(stderr(&output).contains("уже инициализировано"));
}

#[test]
fn status_json_after_init() {
    let root = temp_root();
    let config = init_root(&root);

    let output = run(&config, &["--json", "status"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON status");
    assert!(json["node_id"].as_str().is_some_and(|s| !s.is_empty()));
    assert_eq!(json["pending_actions"], "0");
    assert_eq!(json["notes"], "0");
}

#[test]
fn notes_roundtrip() {
    let root = temp_root();
    let config = init_root(&root);

    // Создание.
    let output = run(
        &config,
        &[
            "--json",
            "notes",
            "create",
            "Test note",
            "--body",
            "Hello",
            "--tag",
            "demo",
        ],
    );
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let note: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON note");
    let id = note["id"].as_str().expect("id заметки").to_string();

    // Список и поиск.
    let output = run(&config, &["--json", "notes", "list"]);
    let list: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON list");
    assert_eq!(list.as_array().map(Vec::len), Some(1));

    let output = run(&config, &["--json", "notes", "search", "Test"]);
    let found: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON search");
    assert_eq!(found.as_array().map(Vec::len), Some(1));

    // Удаление (мягкое) и пустой список.
    let output = run(&config, &["notes", "delete", &id]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let output = run(&config, &["--json", "notes", "list"]);
    let list: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON list 2");
    assert_eq!(list.as_array().map(Vec::len), Some(0));
}

#[test]
fn notes_delete_bad_id_reports_error() {
    let root = temp_root();
    let config = init_root(&root);

    let output = run(&config, &["notes", "delete", "not-a-uuid"]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("некорректный id"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn config_set_and_get_roundtrip() {
    let root = temp_root();
    let config = init_root(&root);

    let output = run(&config, &["config", "set", "general.language", "en"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let output = run(&config, &["config", "get", "general.language"]);
    assert!(output.status.success());
    assert_eq!(stdout(&output).trim(), "en");

    // Неизвестный ключ — ошибка.
    let output = run(&config, &["config", "get", "general.unknown"]);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("ключ не найден"));
}

#[test]
fn approval_and_nodes_empty_after_init() {
    let root = temp_root();
    let config = init_root(&root);

    let output = run(&config, &["--json", "approval", "list"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "[]");

    let output = run(&config, &["--json", "nodes", "list"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output).trim(), "[]");

    let output = run(&config, &["nodes", "heavy"]);
    assert!(output.status.success());
    assert!(
        stdout(&output).contains("нет"),
        "stdout: {}",
        stdout(&output)
    );
}

#[test]
fn benchmark_json_runs() {
    let root = temp_root();
    let config = init_root(&root);

    let output = run(&config, &["--json", "benchmark"]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let json: serde_json::Value = serde_json::from_str(&stdout(&output)).expect("JSON benchmark");
    assert!(json["cpu_score"].as_u64().is_some(), "JSON: {json}");
    assert!(json["memory_bandwidth_mbps"].as_f64().is_some());
}

#[test]
fn stubs_report_unimplemented_clearly() {
    let root = temp_root();
    let config = root.join("config.toml");

    for args in [
        vec!["query", "привет"],
        vec!["start"],
        vec!["sync", "status"],
        vec!["email", "folders", "work"],
    ] {
        let output = run(&config, &args);
        assert!(!output.status.success(), "должна быть ошибка: {args:?}");
        assert!(
            stderr(&output).contains("не реализовано"),
            "stderr для {args:?}: {}",
            stderr(&output)
        );
    }
}
