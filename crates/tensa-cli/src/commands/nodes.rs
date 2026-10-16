//! `tensa nodes list/get/heavy` (сеть узлов: реестр и heavy-выбор).

use tensa_nodes::{Node, NodeRegistry};

use crate::cli::{NodesArgs, NodesCommand};
use crate::commands::{enum_name, ts, Ctx};
use crate::error::{parse_id, CliError, Result};

/// Выполнить `nodes`.
pub async fn run(ctx: &Ctx, args: &NodesArgs) -> Result<()> {
    let registry = ctx.nodes().await?;
    match &args.command {
        NodesCommand::List => {
            let nodes = registry.list_all().await?;
            if ctx.output.json_mode() {
                return ctx.output.json(&nodes);
            }
            if nodes.is_empty() {
                ctx.output.message("реестр узлов пуст");
                return Ok(());
            }
            let rows: Vec<Vec<String>> = nodes.iter().map(node_row).collect();
            ctx.output.table(
                &[
                    "node_id",
                    "имя",
                    "роль",
                    "онлайн",
                    "RAM свободно, МБ",
                    "связь",
                ],
                &rows,
            );
        }
        NodesCommand::Get { id } => {
            let id = parse_id(id)?;
            let node = registry
                .get(id)
                .await?
                .ok_or_else(|| CliError::Message(format!("узел {id} не найден")))?;
            if ctx.output.json_mode() {
                return ctx.output.json(&node);
            }
            ctx.output.key_values(&node_pairs(&node));
        }
        NodesCommand::Heavy => match registry.find_heavy().await? {
            Some(node) => {
                if ctx.output.json_mode() {
                    return ctx.output.json(&node);
                }
                ctx.output.success("heavy-узел для инференса найден");
                ctx.output.key_values(&node_pairs(&node));
            }
            None => {
                if ctx.output.json_mode() {
                    return ctx.output.json(&serde_json::Value::Null);
                }
                ctx.output.message("онлайн heavy-узла нет");
            }
        },
    }
    Ok(())
}

/// Строка таблицы узлов.
fn node_row(node: &Node) -> Vec<String> {
    vec![
        node.node_id.to_string(),
        node.name.clone(),
        enum_name(&node.role),
        if node.online { "да" } else { "нет" }.to_string(),
        node.ram_available_mb.to_string(),
        ts(node.last_seen),
    ]
}

/// Пары «ключ — значение» карточки узла.
fn node_pairs(node: &Node) -> Vec<(String, String)> {
    vec![
        ("node_id".to_string(), node.node_id.to_string()),
        ("имя".to_string(), node.name.clone()),
        ("роль".to_string(), enum_name(&node.role)),
        (
            "онлайн".to_string(),
            if node.online { "да" } else { "нет" }.to_string(),
        ),
        (
            "RAM всего/свободно, МБ".to_string(),
            format!("{} / {}", node.ram_total_mb, node.ram_available_mb),
        ),
        (
            "CPU".to_string(),
            format!("{} ядер, {:.0}%", node.cpu_cores, node.cpu_usage_percent),
        ),
        (
            "GPU/NPU".to_string(),
            format!(
                "{}/{}",
                if node.has_gpu { "есть" } else { "нет" },
                if node.has_npu { "есть" } else { "нет" }
            ),
        ),
        (
            "батарея".to_string(),
            node.battery_percent
                .map(|percent| format!("{percent}%"))
                .unwrap_or_else(|| "—".to_string()),
        ),
        ("связь".to_string(), ts(node.last_seen)),
    ]
}
