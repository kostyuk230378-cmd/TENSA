//! `GET /events` — Server-Sent Events (Контракт 12):
//! приветственное событие, снимок здоровья и трансляция изменений ядра
//! через [`tokio::sync::broadcast`].

use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use tokio_stream::wrappers::errors::BroadcastStreamRecvError;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::{once, Stream, StreamExt};

use crate::routes::health;
use crate::state::{AppState, ServerEvent};

/// Интервал keep-alive комментариев (защита от таймаутов прокси).
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

/// `GET /events` — поток событий PWA.
///
/// Поток собирается из трёх частей: приветственное событие, текущий снимок
/// здоровья и далее — всё, что публикуется в канал событий (например,
/// `approval:updated` после approve/reject).
pub async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = std::result::Result<Event, std::convert::Infallible>>> {
    let welcome = ServerEvent::welcome(health::VERSION);
    let health_event = status_health(&state).await;

    let receiver = state.subscribe();
    let live = BroadcastStream::new(receiver).filter_map(|item| match item {
        Ok(event) => Some(event),
        Err(BroadcastStreamRecvError::Lagged(skipped)) => {
            tracing::warn!(skipped, "SSE-подписчик отстал, часть событий пропущена");
            None
        }
    });

    let stream = once(welcome)
        .chain(once(health_event))
        .chain(live)
        .map(to_sse_event);

    Sse::new(stream).keep_alive(KeepAlive::new().interval(KEEP_ALIVE_INTERVAL))
}

/// Текущее состояние ядра, упакованное в событие `status:health`.
async fn status_health(state: &AppState) -> ServerEvent {
    let status = health::snapshot(state).await;
    match serde_json::to_value(&status) {
        Ok(data) => ServerEvent::new(ServerEvent::STATUS_HEALTH, data),
        Err(err) => {
            tracing::warn!(error = %err, "не удалось сериализовать снимок здоровья для SSE");
            ServerEvent::new(ServerEvent::STATUS_HEALTH, serde_json::Value::Null)
        }
    }
}

/// Преобразование внутреннего события в кадр SSE.
///
/// JSON-полезная нагрузка отдаётся текстом внутри `data`, чтобы клиент
/// читал доменные поля без двойного кодирования.
fn to_sse_event(event: ServerEvent) -> std::result::Result<Event, std::convert::Infallible> {
    Ok(Event::default()
        .event(event.event)
        .data(event.data.to_string()))
}
