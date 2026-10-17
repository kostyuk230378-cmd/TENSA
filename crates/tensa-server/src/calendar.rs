//! Календарь ядра (`/api/v1/calendar/*`, Контракт 7): события, guard
//! конфликтов, экспорт ICS.
//!
//! Хранилище — JSON-файл `<root>/calendar.json` ([derived]: SQLite/CalDAV
//! придут отдельным этапом; для MVP важны контрактные поля и guard).
//! Все операции сериализуются мьютексом, файл пишется атомарно (tmp+rename).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

/// Событие календаря.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Идентификатор.
    pub id: Uuid,
    /// Заголовок.
    pub title: String,
    /// Начало (unix seconds).
    pub starts_at: i64,
    /// Конец (unix seconds).
    pub ends_at: i64,
    /// Место (опционально).
    pub location: Option<String>,
    /// Заметки (опционально).
    pub notes: Option<String>,
    /// Когда создано (unix seconds).
    pub created_at: i64,
}

/// Новое событие из запроса.
#[derive(Debug, Clone, Deserialize)]
pub struct NewEvent {
    /// Заголовок.
    pub title: String,
    /// Начало (unix seconds).
    pub starts_at: i64,
    /// Конец (unix seconds).
    pub ends_at: i64,
    /// Место.
    #[serde(default)]
    pub location: Option<String>,
    /// Заметки.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Ошибки календаря.
#[derive(Debug, thiserror::Error)]
pub enum CalendarError {
    /// Ошибка файла состояния.
    #[error("календарь: {0}")]
    Storage(String),
    /// Некорректный интервал/поля.
    #[error("календарь: {0}")]
    BadInput(String),
    /// Пересечение с существующими событиями.
    #[error("конфликт: пересечение с {}", .0.len())]
    Conflict(Vec<Event>),
}

/// Результат операций календаря.
pub type Result<T> = std::result::Result<T, CalendarError>;

/// Хранилище событий (JSON-файл или память для тестов).
pub struct CalendarStore {
    path: Option<PathBuf>,
    events: Mutex<Option<Vec<Event>>>,
}

impl CalendarStore {
    /// Хранилище в файле (создаётся при первой записи).
    pub fn new(path: PathBuf) -> Self {
        Self {
            path: Some(path),
            events: Mutex::new(None),
        }
    }

    /// Хранилище в памяти (тесты, состояние без календаря).
    pub fn in_memory() -> Self {
        Self {
            path: None,
            events: Mutex::new(Some(Vec::new())),
        }
    }

    /// Загрузить события из файла, если ещё не загружены.
    async fn ensure_loaded(&self, guard: &mut Option<Vec<Event>>) -> Result<()> {
        if guard.is_some() {
            return Ok(());
        }
        let events = match &self.path {
            Some(path) if path.exists() => {
                let raw = std::fs::read_to_string(path).map_err(|e| {
                    CalendarError::Storage(format!("чтение {}: {e}", path.display()))
                })?;
                serde_json::from_str(&raw).map_err(|e| {
                    CalendarError::Storage(format!("разбор {}: {e}", path.display()))
                })?
            }
            _ => Vec::new(),
        };
        *guard = Some(events);
        Ok(())
    }

    /// Записать состояние в файл (атомарно) или оставить в памяти.
    async fn persist(&self, events: &[Event]) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let body = serde_json::to_string_pretty(events)
            .map_err(|e| CalendarError::Storage(format!("сериализация: {e}")))?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                CalendarError::Storage(format!("каталог {}: {e}", parent.display()))
            })?;
        }
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, body)
            .map_err(|e| CalendarError::Storage(format!("запись {}: {e}", tmp.display())))?;
        std::fs::rename(&tmp, path).map_err(|e| {
            CalendarError::Storage(format!("переименование {}: {e}", path.display()))
        })?;
        Ok(())
    }

    /// Список событий (опционально — пересекающих окно [from, to]).
    pub async fn list(&self, from: Option<i64>, to: Option<i64>) -> Result<Vec<Event>> {
        let mut guard = self.events.lock().await;
        self.ensure_loaded(&mut guard).await?;
        let events = guard.as_ref().cloned().unwrap_or_default();
        let mut filtered: Vec<Event> = match (from, to) {
            (Some(from), Some(to)) => events
                .into_iter()
                .filter(|event| event.starts_at < to && event.ends_at > from)
                .collect(),
            (Some(from), None) => events
                .into_iter()
                .filter(|event| event.ends_at > from)
                .collect(),
            (None, Some(to)) => events
                .into_iter()
                .filter(|event| event.starts_at < to)
                .collect(),
            (None, None) => events,
        };
        filtered.sort_by_key(|event| event.starts_at);
        Ok(filtered)
    }

    /// Пересечения с интервалом [starts, ends).
    pub async fn conflicts(&self, starts: i64, ends: i64) -> Result<Vec<Event>> {
        if ends <= starts {
            return Err(CalendarError::BadInput(
                "ends_at должен быть больше starts_at".into(),
            ));
        }
        let mut guard = self.events.lock().await;
        self.ensure_loaded(&mut guard).await?;
        let events = guard.as_ref().cloned().unwrap_or_default();
        Ok(events
            .into_iter()
            .filter(|event| event.starts_at < ends && event.ends_at > starts)
            .collect())
    }

    /// Создать событие; при пересечении — [`CalendarError::Conflict`].
    pub async fn create(&self, new: NewEvent) -> Result<Event> {
        if new.title.trim().is_empty() {
            return Err(CalendarError::BadInput("title обязателен".into()));
        }
        if new.ends_at <= new.starts_at {
            return Err(CalendarError::BadInput(
                "ends_at должен быть больше starts_at".into(),
            ));
        }
        let mut guard = self.events.lock().await;
        self.ensure_loaded(&mut guard).await?;
        let events = guard
            .as_mut()
            .ok_or_else(|| CalendarError::Storage("состояние".into()))?;
        let overlaps: Vec<Event> = events
            .iter()
            .filter(|event| event.starts_at < new.ends_at && event.ends_at > new.starts_at)
            .cloned()
            .collect();
        if !overlaps.is_empty() {
            return Err(CalendarError::Conflict(overlaps));
        }
        let event = Event {
            id: Uuid::new_v4(),
            title: new.title.trim().to_string(),
            starts_at: new.starts_at,
            ends_at: new.ends_at,
            location: new.location,
            notes: new.notes,
            created_at: chrono::Utc::now().timestamp(),
        };
        events.push(event.clone());
        self.persist(events).await?;
        Ok(event)
    }

    /// Удалить событие; `false` — не найдено.
    pub async fn delete(&self, id: Uuid) -> Result<bool> {
        let mut guard = self.events.lock().await;
        self.ensure_loaded(&mut guard).await?;
        let events = guard
            .as_mut()
            .ok_or_else(|| CalendarError::Storage("состояние".into()))?;
        let before = events.len();
        events.retain(|event| event.id != id);
        let removed = events.len() != before;
        if removed {
            self.persist(events).await?;
        }
        Ok(removed)
    }

    /// Экспорт всех событий в ICS (text/calendar).
    pub async fn ics(&self) -> Result<String> {
        let events = self.list(None, None).await?;
        let mut out = String::from(
            "BEGIN:VCALENDAR\r\nVERSION:2.0\r\nPRODID:-//Tensa//Calendar//RU\r\nCALSCALE:GREGORIAN\r\n",
        );
        for event in events {
            out.push_str("BEGIN:VEVENT\r\n");
            out.push_str(&format!("UID:{}\r\n", event.id));
            out.push_str(&format!("DTSTAMP:{}\r\n", ics_time(event.created_at)));
            out.push_str(&format!("DTSTART:{}\r\n", ics_time(event.starts_at)));
            out.push_str(&format!("DTEND:{}\r\n", ics_time(event.ends_at)));
            out.push_str(&format!("SUMMARY:{}\r\n", ics_escape(&event.title)));
            if let Some(location) = &event.location {
                out.push_str(&format!("LOCATION:{}\r\n", ics_escape(location)));
            }
            if let Some(notes) = &event.notes {
                out.push_str(&format!("DESCRIPTION:{}\r\n", ics_escape(notes)));
            }
            out.push_str("END:VEVENT\r\n");
        }
        out.push_str("END:VCALENDAR\r\n");
        Ok(out)
    }
}

/// Время в формате ICS (UTC).
fn ics_time(seconds: i64) -> String {
    chrono::DateTime::from_timestamp(seconds, 0)
        .unwrap_or_else(|| chrono::DateTime::from_timestamp(0, 0).unwrap_or_default())
        .format("%Y%m%dT%H%M%SZ")
        .to_string()
}

/// Экранирование текста ICS (RFC 5545).
fn ics_escape(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace(';', "\\;")
        .replace(',', "\\,")
        .replace('\n', "\\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Событие на интервале [start, start+duration).
    fn new_event(title: &str, start: i64, duration: i64) -> NewEvent {
        NewEvent {
            title: title.to_string(),
            starts_at: start,
            ends_at: start + duration,
            location: None,
            notes: None,
        }
    }

    #[tokio::test]
    async fn create_list_delete_roundtrip() {
        let store = CalendarStore::in_memory();
        let event = store
            .create(new_event("Планёрка", 1_000, 3_600))
            .await
            .expect("создание");
        let events = store.list(None, None).await.expect("список");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].id, event.id);
        assert!(store.delete(event.id).await.expect("удаление"));
        assert!(store.list(None, None).await.expect("список").is_empty());
    }

    #[tokio::test]
    async fn conflict_detected_and_reported() {
        let store = CalendarStore::in_memory();
        store
            .create(new_event("Встреча A", 0, 3_600))
            .await
            .expect("A");
        let error = store
            .create(new_event("Встреча B", 1_800, 3_600))
            .await
            .expect_err("должен быть конфликт");
        match error {
            CalendarError::Conflict(events) => {
                assert_eq!(events.len(), 1);
                assert_eq!(events[0].title, "Встреча A");
            }
            other => panic!("ожидался Conflict, получено: {other:?}"),
        }
    }

    #[tokio::test]
    async fn adjacent_events_do_not_conflict() {
        let store = CalendarStore::in_memory();
        store.create(new_event("A", 0, 3_600)).await.expect("A");
        store.create(new_event("B", 3_600, 3_600)).await.expect("B");
        assert_eq!(store.list(None, None).await.expect("список").len(), 2);
    }

    #[tokio::test]
    async fn bad_interval_rejected() {
        let store = CalendarStore::in_memory();
        let error = store
            .create(new_event("Кривой", 100, -50))
            .await
            .expect_err("должна быть ошибка");
        assert!(matches!(error, CalendarError::BadInput(_)));
    }

    #[tokio::test]
    async fn ics_contains_event_and_escapes() {
        let store = CalendarStore::in_memory();
        store
            .create(NewEvent {
                title: "Совещание, важное; срочно".to_string(),
                starts_at: 1_700_000_000,
                ends_at: 1_700_003_600,
                location: Some("Офис".to_string()),
                notes: None,
            })
            .await
            .expect("создание");
        let ics = store.ics().await.expect("ics");
        assert!(ics.contains("BEGIN:VCALENDAR"));
        assert!(ics.contains("SUMMARY:Совещание\\, важное\\; срочно"));
        assert!(ics.contains("DTSTART:20231114T221320Z"));
    }

    #[tokio::test]
    async fn file_store_survives_reload() {
        let dir = std::env::temp_dir().join(format!("tensa-calendar-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).expect("каталог");
        let path = dir.join("calendar.json");
        {
            let store = CalendarStore::new(path.clone());
            store
                .create(new_event("Постоянное", 0, 60))
                .await
                .expect("create");
        }
        let reopened = CalendarStore::new(path);
        let events = reopened.list(None, None).await.expect("list");
        assert_eq!(events.len(), 1);
        let _ = std::fs::remove_dir_all(dir);
    }
}
