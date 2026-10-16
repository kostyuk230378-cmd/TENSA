//! Sync-адаптеры заметок и контактов: мост между Store-трейтами tensa-core
//! и sync-событиями (Контракт 8: NoteEvent/ContactEvent).
//!
//! Одобрено Архитектором 2026-10-15: методы `get_notes_since`/`count_notes`
//! и `get_contacts_since`/`count_contacts` добавлены в Контракт 3.
//! `NoopSyncSource` (service.rs) остаётся fallback.

use crate::error::{Result, SyncError};
use crate::service::{ContactEvent, ContactsSyncSource, NoteEvent, NotesSyncSource};
use async_trait::async_trait;
use std::sync::Arc;
use tensa_core::{Contact, ContactsStore, Note, NotesStore};

fn store_err(e: impl std::fmt::Display) -> SyncError {
    SyncError::Store(e.to_string())
}

/// Адаптер заметок: события из `NotesStore`, применение — через LWW-реализацию
/// `update_note` (Контракт 3).
pub struct StoreNotesSync {
    store: Arc<dyn NotesStore>,
}

impl StoreNotesSync {
    pub fn new(store: Arc<dyn NotesStore>) -> Self {
        Self { store }
    }

    fn note_to_event(note: &Note) -> Result<NoteEvent> {
        // created/updated различаем по времени; deleted не приходит
        // (get_notes_since фильтрует deleted = 0, Контракт 3).
        let event_type = if note.updated_at > note.created_at {
            "updated"
        } else {
            "created"
        };
        Ok(NoteEvent {
            note_id: note.id,
            event_type: event_type.into(),
            note_data: serde_json::to_value(note)?,
            timestamp: note.updated_at,
            node_id: note.updated_on_node.clone(),
            vector_clock: note.vector_clock.clone(),
        })
    }
}

#[async_trait]
impl NotesSyncSource for StoreNotesSync {
    async fn events_since(&self, last_updated_at: i64, limit: usize) -> Result<Vec<NoteEvent>> {
        let notes = self
            .store
            .get_notes_since(last_updated_at)
            .await
            .map_err(store_err)?;
        notes.iter().take(limit).map(Self::note_to_event).collect()
    }

    async fn apply_event(&self, event: NoteEvent) -> Result<()> {
        let remote: Note = serde_json::from_value(event.note_data.clone())
            .map_err(|e| SyncError::Store(format!("bad note snapshot: {e}")))?;
        match self.store.get_note(remote.id).await.map_err(store_err)? {
            Some(_) => {
                // update_note сам решает LWW (Контракт 3) и тикает векторные часы.
                self.store.update_note(remote).await.map_err(store_err)?;
                Ok(())
            }
            None => Err(SyncError::Store(format!(
                "note {} not present locally: insert-with-id is not part of NotesStore \
                 (pending architecture decision); event not applied",
                remote.id
            ))),
        }
    }
}

/// Адаптер контактов: симметричен заметкам.
pub struct StoreContactsSync {
    store: Arc<dyn ContactsStore>,
}

impl StoreContactsSync {
    pub fn new(store: Arc<dyn ContactsStore>) -> Self {
        Self { store }
    }

    fn contact_to_event(contact: &Contact) -> Result<ContactEvent> {
        let event_type = if contact.updated_at > contact.created_at {
            "updated"
        } else {
            "created"
        };
        Ok(ContactEvent {
            contact_id: contact.id,
            event_type: event_type.into(),
            contact_data: serde_json::to_value(contact)?,
            timestamp: contact.updated_at,
            node_id: contact.updated_on_node.clone(),
            vector_clock: contact.vector_clock.clone(),
        })
    }
}

#[async_trait]
impl ContactsSyncSource for StoreContactsSync {
    async fn events_since(&self, last_updated_at: i64, limit: usize) -> Result<Vec<ContactEvent>> {
        let contacts = self
            .store
            .get_contacts_since(last_updated_at)
            .await
            .map_err(store_err)?;
        contacts
            .iter()
            .take(limit)
            .map(Self::contact_to_event)
            .collect()
    }

    async fn apply_event(&self, event: ContactEvent) -> Result<()> {
        let remote: Contact = serde_json::from_value(event.contact_data.clone())
            .map_err(|e| SyncError::Store(format!("bad contact snapshot: {e}")))?;
        match self.store.get_contact(remote.id).await.map_err(store_err)? {
            Some(_) => {
                self.store.update_contact(remote).await.map_err(store_err)?;
                Ok(())
            }
            None => Err(SyncError::Store(format!(
                "contact {} not present locally: insert-with-id is not part of ContactsStore \
                 (pending architecture decision); event not applied",
                remote.id
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tensa_core::{NewContact, NewNote, SqliteContactsStore, SqliteNotesStore};

    #[tokio::test]
    async fn notes_events_since_and_lww_apply() {
        let notes = Arc::new(SqliteNotesStore::new_in_memory().await.expect("store"));
        let adapter = StoreNotesSync::new(notes.clone());

        let note = notes
            .create_note(NewNote {
                title: "a".into(),
                body: "b".into(),
                tags: vec![],
            })
            .await
            .expect("create");

        // Чтение: событие created.
        let events = adapter.events_since(0, 100).await.expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "created");
        assert_eq!(events[0].note_id, note.id);

        // Чтение по watermark: событий нет.
        assert!(adapter
            .events_since(note.updated_at, 100)
            .await
            .expect("none")
            .is_empty());

        // Применение: обновление с новым updated_at (LWW пропустит через update_note).
        let mut remote = note.clone();
        remote.title = "remote".into();
        remote.updated_at = note.updated_at + 10;
        remote.updated_on_node = "other-node".into();
        let event = NoteEvent {
            note_id: remote.id,
            event_type: "updated".into(),
            note_data: serde_json::to_value(&remote).expect("snapshot"),
            timestamp: remote.updated_at,
            node_id: "other-node".into(),
            vector_clock: remote.vector_clock.clone(),
        };
        adapter.apply_event(event).await.expect("apply");
        let local = notes.get_note(note.id).await.expect("get").expect("exists");
        assert_eq!(local.title, "remote");
    }

    #[tokio::test]
    async fn notes_apply_missing_is_explicit_error() {
        let notes = Arc::new(SqliteNotesStore::new_in_memory().await.expect("store"));
        let adapter = StoreNotesSync::new(notes);
        let ghost = Note {
            id: uuid::Uuid::new_v4(),
            title: "x".into(),
            body: "y".into(),
            tags: vec![],
            created_at: 0,
            updated_at: 1,
            created_on_node: "n".into(),
            updated_on_node: "n".into(),
            deleted: false,
            vector_clock: tensa_approval::VectorClock::new(),
        };
        let event = NoteEvent {
            note_id: ghost.id,
            event_type: "updated".into(),
            note_data: serde_json::to_value(&ghost).expect("snapshot"),
            timestamp: 1,
            node_id: "n".into(),
            vector_clock: tensa_approval::VectorClock::new(),
        };
        let err = adapter.apply_event(event).await.expect_err("missing");
        assert!(matches!(err, SyncError::Store(_)), "got: {err:?}");
        assert!(err.to_string().contains("insert-with-id"));
    }

    #[tokio::test]
    async fn contacts_events_since_roundtrip() {
        let contacts = Arc::new(SqliteContactsStore::new_in_memory().await.expect("store"));
        let adapter = StoreContactsSync::new(contacts.clone());
        contacts
            .create_contact(NewContact {
                name: "Иван".into(),
                email: Some("i@e.com".into()),
                phone: None,
                telegram: None,
                whatsapp: None,
                notes: None,
                tags: vec![],
            })
            .await
            .expect("create");

        let events = adapter.events_since(0, 100).await.expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "created");
        assert!(events[0].contact_data.get("name").is_some());
    }
}
