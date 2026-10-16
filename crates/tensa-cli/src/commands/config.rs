//! `tensa config get/set/list/reload` (Контракт 4: `config.toml`).
//!
//! `set` пишет файл через `Config::save` (валидация + бэкап `*.toml.bak`).

use crate::cli::{ConfigArgs, ConfigCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};
use tensa_core::Config;

/// Выполнить `config`.
pub async fn run(ctx: &Ctx, args: &ConfigArgs) -> Result<()> {
    match &args.command {
        ConfigCommand::List => list(ctx),
        ConfigCommand::Get { key } => get(ctx, key),
        ConfigCommand::Set { key, value } => set(ctx, key, value),
        ConfigCommand::Reload => reload(ctx),
    }
}

/// Ошибка сериализации JSON в понятном виде.
fn json_err(error: serde_json::Error) -> CliError {
    CliError::Message(format!("сериализация JSON: {error}"))
}

/// Конфиг как `toml::Value` (для get/set по точечным ключам).
fn config_value(ctx: &Ctx) -> Result<toml::Value> {
    let config = ctx.config()?;
    toml::Value::try_from(&config).map_err(|e| CliError::Message(format!("конфиг → TOML: {e}")))
}

/// Обход `toml::Value` по точечному ключу.
fn lookup<'a>(root: &'a toml::Value, key: &str) -> Option<&'a toml::Value> {
    let mut current = root;
    for part in key.split('.') {
        current = current.get(part)?;
    }
    Some(current)
}

/// Человекочитаемое значение TOML (строки — без кавычек).
fn render(value: &toml::Value) -> String {
    match value {
        toml::Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

/// Разбор значения из CLI: bool → integer → float → строка.
fn parse_value(raw: &str) -> toml::Value {
    if let Ok(boolean) = raw.parse::<bool>() {
        return toml::Value::Boolean(boolean);
    }
    if let Ok(integer) = raw.parse::<i64>() {
        return toml::Value::Integer(integer);
    }
    if let Ok(float) = raw.parse::<f64>() {
        return toml::Value::Float(float);
    }
    toml::Value::String(raw.to_string())
}

/// Записать значение по точечному ключу (создавая отсутствующие таблицы).
fn set_path(root: &mut toml::Value, key: &str, value: toml::Value) -> Result<()> {
    let parts: Vec<&str> = key.split('.').collect();
    let (last, parents) = parts
        .split_last()
        .ok_or_else(|| CliError::Message("пустой ключ".to_string()))?;

    let mut current = root;
    for part in parents {
        let table = current
            .as_table_mut()
            .ok_or_else(|| CliError::Message(format!("«{part}» — не таблица")))?;
        current = table
            .entry((*part).to_string())
            .or_insert_with(|| toml::Value::Table(toml::Table::new()));
    }
    let table = current
        .as_table_mut()
        .ok_or_else(|| CliError::Message(format!("«{key}» — не таблица")))?;
    table.insert((*last).to_string(), value);
    Ok(())
}

fn list(ctx: &Ctx) -> Result<()> {
    let config = ctx.config()?;
    if ctx.output.json_mode() {
        return ctx
            .output
            .json(&serde_json::to_value(&config).map_err(json_err)?);
    }
    let text =
        toml::to_string_pretty(&config).map_err(|e| CliError::Message(format!("TOML: {e}")))?;
    ctx.output.message(text.trim_end());
    Ok(())
}

fn get(ctx: &Ctx, key: &str) -> Result<()> {
    let value = config_value(ctx)?;
    let found =
        lookup(&value, key).ok_or_else(|| CliError::Message(format!("ключ не найден: {key}")))?;
    if ctx.output.json_mode() {
        return ctx
            .output
            .json(&serde_json::to_value(found).map_err(json_err)?);
    }
    ctx.output.message(render(found));
    Ok(())
}

fn set(ctx: &Ctx, key: &str, raw: &str) -> Result<()> {
    let mut root = config_value(ctx)?;
    let value = parse_value(raw);
    set_path(&mut root, key, value)?;

    let updated: Config = root
        .try_into()
        .map_err(|e| CliError::Message(format!("значение несовместимо с конфигом: {e}")))?;
    updated.save(&ctx.config_path)?;

    if ctx.output.json_mode() {
        return ctx
            .output
            .json(&serde_json::json!({ "updated": key, "value": raw }));
    }
    ctx.output
        .success(format!("обновлено: {key} = {raw} (бэкап: *.toml.bak)"));
    Ok(())
}

fn reload(ctx: &Ctx) -> Result<()> {
    // CLI — отдельный процесс без собственного кэша: reload = строгая
    // проверка файла (загрузка + валидация).
    let config = ctx.config()?;
    if ctx.output.json_mode() {
        return ctx.output.json(&serde_json::json!({
            "reloaded": true,
            "config": ctx.config_path.display().to_string(),
            "node_id": config.general.node_id,
        }));
    }
    ctx.output.success(format!(
        "конфигурация валидна: {} (CLI не кэширует её между запусками)",
        ctx.config_path.display()
    ));
    Ok(())
}
