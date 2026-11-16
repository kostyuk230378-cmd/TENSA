//! `tensa sync ...` — статус узлов и обмен через gRPC (ADR-012, ADR-013,
//! Контракт 8). gRPC-сервер живёт в процессе `tensa-server` (стартует при
//! `[sync] enabled = true`), CLI — клиент: статус реестра, push/pull
//! заметок и контактов (LWW по updated_at, ADR-013 on-demand).

use std::str::FromStr;
use std::sync::Arc;

use tensa_nodes::profile::StubProfiler;
use tensa_nodes::NodeRegistry;
use tensa_nodes::SqliteNodeRegistry;
use tensa_sync::adapters::StoreNotesSync;
use tensa_sync::client::GrpcSyncService;
use tensa_sync::{NotesSyncSource, SyncService};

use crate::cli::{SyncArgs, SyncCommand};
use crate::commands::Ctx;
use crate::error::{CliError, Result};

/// Выполнить `sync`.
pub async fn run(ctx: &Ctx, args: &SyncArgs) -> Result<()> {
    match &args.command {
        SyncCommand::Status => status(ctx).await,
        SyncCommand::Push => transfer(ctx, args).await,
        SyncCommand::Pull => transfer(ctx, args).await,
        SyncCommand::Start | SyncCommand::Stop => {
            // Жизненный цикл синхронизации — у сервера: gRPC-сервер
            // (порт из [sync] grpc_port) стартует вместе с tensa-server
            // при enabled = true. Отдельный демон не нужен.
            println!(
                "Синхронизация управляется сервером: запустите tensa-server \
                 с [sync] enabled = true (gRPC-порт — из конфига)."
            );
            Ok(())
        }
    }
}

/// `tensa sync status` — узлы реестра и настройки синхронизации.
async fn status(ctx: &Ctx) -> Result<()> {
    let registry = SqliteNodeRegistry::new(&ctx.paths.nodes_db())
        .await
        .map_err(|error| CliError::Message(format!("реестр узлов: {error}")))?;
    let nodes = registry
        .list_all()
        .await
        .map_err(|error| CliError::Message(format!("реестр узлов: {error}")))?;

    let config = tensa_core::Config::load(&ctx.config_path)?;
    println!(
        "Синхронизация: enabled = {}, gRPC-порт = {}, Tailscale = {} ({})",
        config.sync.enabled,
        config.sync.grpc_port,
        config.sync.tailscale_enabled,
        config.sync.tailscale_hostname,
    );
    println!("Узлов в реестре: {}", nodes.len());
    for node in &nodes {
        println!(
            "  {} | {} | {:?}/{:?} | {} | online = {} | last_seen = {}",
            node.node_id,
            node.name,
            node.device_type,
            node.role,
            node.tailscale_ip,
            node.online,
            node.last_seen,
        );
    }
    if nodes.is_empty() {
        println!("  (узлы появляются в реестре, когда клиенты регистрируются по gRPC)");
    }
    Ok(())
}

/// `tensa sync push|pull --node <host:port>` — обмен заметками и контактами
/// (LWW, идемпотентность — на стороне сервера через operation_id).
async fn transfer(ctx: &Ctx, args: &SyncArgs) -> Result<()> {
    let Some(node) = &args.node else {
        return Err(CliError::Message(
            "укажите узел: --node <host:port> (например, 100.64.0.1:50051)".to_string(),
        ));
    };
    let config = tensa_core::Config::load(&ctx.config_path)?;
    let endpoint = tonic::transport::Endpoint::from_shared(format!("http://{node}"))
        .map_err(|error| CliError::Message(format!("адрес узла «{node}»: {error}")))?;
    let service = GrpcSyncService::new(
        endpoint,
        config.general.node_id.clone(),
        Arc::new(StubProfiler::desktop()),
    );
    let notes =
        tensa_core::SqliteNotesStore::new(&ctx.paths.notes_db(), config.general.node_id.clone())
            .await
            .map_err(|error| CliError::Message(format!("заметки: {error}")))?;
    let source = StoreNotesSync::new(Arc::new(notes));

    match &args.command {
        SyncCommand::Push => {
            // Заметки, изменённые с начала времён (MVP: полный проход, LWW
            // на принимающей стороне отбрасывает устаревшие версии).
            let events = source
                .events_since(0, 10_000)
                .await
                .map_err(|error| CliError::Message(format!("заметки: {error}")))?;
            let total = events.len();
            for event in events {
                service
                    .push_note_event(event)
                    .await
                    .map_err(|error| CliError::Message(format!("push: {error}")))?;
            }
            println!("Отправлено событий заметок: {total}");
        }
        SyncCommand::Pull => {
            // DataService.GetNotes удалённого узла → LWW-применение локально.
            let mut client = tensa_sync::grpc::proto::tensa::data::data_service_client::DataServiceClient::connect(
                tonic::transport::Endpoint::from_shared(format!("http://{node}"))
                    .map_err(|error| CliError::Message(format!("адрес узла «{node}»: {error}")))?,
            )
            .await
            .map_err(|error| CliError::Message(format!("подключение: {error}")))?;
            let reply = client
                .get_notes(tensa_sync::grpc::proto::tensa::data::GetNotesRequest {
                    node_id: config.general.node_id.clone(),
                    limit: 1000,
                    offset: 0,
                })
                .await
                .map_err(|error| CliError::Message(format!("pull: {error}")))?
                .into_inner();
            let mut applied = 0;
            for note in &reply.notes {
                let event = note_event_from_proto(note);
                if source.apply_event(event).await.is_ok() {
                    applied += 1;
                }
            }
            println!("Принято заметок: {} из {}", applied, reply.notes.len());
        }
        _ => unreachable!("start/stop обработаны выше"),
    }
    Ok(())
}

/// Событие из proto-заметки удалённого узла (LWW-применение локально).
fn note_event_from_proto(
    note: &tensa_sync::grpc::proto::tensa::data::Note,
) -> tensa_sync::NoteEvent {
    use tensa_sync::NoteEvent;
    let id = uuid::Uuid::from_str(&note.note_id).unwrap_or_else(|_| uuid::Uuid::new_v4());
    NoteEvent {
        note_id: id,
        event_type: "updated".to_string(),
        note_data: serde_json::json!({
            "id": note.note_id,
            "title": note.title,
            "body": note.body,
            "tags": note.tags,
            "created_at": note.created_at,
            "updated_at": note.updated_at,
            "created_on_node": "remote",
            "updated_on_node": "remote",
            "deleted": false,
            "vector_clock": {},
        }),
        timestamp: note.updated_at,
        node_id: "remote".to_string(),
        vector_clock: tensa_approval::VectorClock::new(),
    }
}
