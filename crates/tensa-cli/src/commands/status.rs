//! `tensa status` — сводка: пути, узел, счётчики подсистем.

use tensa_approval::ApprovalStore;
use tensa_core::NotesStore;
use tensa_nodes::NodeRegistry;

use crate::commands::Ctx;
use crate::error::Result;

/// Выполнить `status`.
pub async fn run(ctx: &Ctx) -> Result<()> {
    let config = ctx.config()?;
    let approvals = ctx.approvals().await?;
    let pending = approvals.get_pending().await?;
    let notes = ctx.notes().await?;
    let notes_count = notes.count_notes().await?;
    let registry = ctx.nodes().await?;
    let nodes = registry.list_all().await?;
    let online = nodes.iter().filter(|node| node.online).count();

    let pairs = vec![
        ("root".to_string(), ctx.paths.root().display().to_string()),
        ("config".to_string(), ctx.config_path.display().to_string()),
        ("node_id".to_string(), config.general.node_id.clone()),
        (
            "language".to_string(),
            format!("{:?}", config.general.language).to_lowercase(),
        ),
        (
            "role".to_string(),
            format!("{:?}", config.general.role).to_lowercase(),
        ),
        (
            "device_type".to_string(),
            format!("{:?}", config.general.device_type).to_lowercase(),
        ),
        ("pending_actions".to_string(), pending.len().to_string()),
        ("notes".to_string(), notes_count.to_string()),
        (
            "nodes".to_string(),
            format!("{} (онлайн: {online})", nodes.len()),
        ),
    ];

    if ctx.output.json_mode() {
        let map: serde_json::Map<String, serde_json::Value> = pairs
            .iter()
            .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
            .collect();
        return ctx.output.json(&serde_json::Value::Object(map));
    }

    ctx.output.key_values(&pairs);
    ctx.output
        .message("режим: in-process (CLI не хранит состояние; демон — tensa-server/apps)");
    Ok(())
}
