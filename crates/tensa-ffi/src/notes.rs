//! `NotesStore` (Контракт 9): заметки поверх tensa-core.

use tensa_core::{NewNote as CoreNewNote, NotesStore as CoreNotesStore, SqliteNotesStore};

use crate::convert::{data_root, load_core_config, parse_uuid, uuid_str};
use crate::error::TensaError;
use crate::types::{NewNote, Note};

/// FFI-обёртка хранилища заметок (`interface NotesStore` в UDL).
pub struct NotesStore {
    inner: SqliteNotesStore,
}

impl NotesStore {
    /// Открыть хранилище (база — `<root>/notes.db`).
    pub async fn new(config_path: String) -> Result<Self, TensaError> {
        let config = load_core_config(&config_path)?;
        let store = SqliteNotesStore::new(
            &data_root(&config_path).join("notes.db"),
            config.general.node_id.clone(),
        )
        .await?;
        Ok(Self { inner: store })
    }

    /// Создать заметку.
    pub async fn create_note(&self, note: NewNote) -> Result<Note, TensaError> {
        let created = self
            .inner
            .create_note(CoreNewNote {
                title: note.title,
                body: note.body,
                tags: note.tags,
            })
            .await?;
        Ok(to_note(created))
    }

    /// Обновить заметку (поля ядра вне UDL сохраняются).
    pub async fn update_note(&self, note: Note) -> Result<Note, TensaError> {
        let id = parse_uuid("notes.update", &note.id)?;
        let mut current = self.inner.get_note(id).await?.ok_or_else(|| {
            TensaError::internal("notes.update", format!("заметка {id} не найдена"))
        })?;
        current.title = note.title;
        current.body = note.body;
        current.tags = note.tags;
        let updated = self.inner.update_note(current).await?;
        Ok(to_note(updated))
    }

    /// Удалить заметку (мягкое удаление, Контракт 3).
    pub async fn delete_note(&self, id: String) -> Result<(), TensaError> {
        let id = parse_uuid("notes.delete", &id)?;
        self.inner.delete_note(id).await?;
        Ok(())
    }

    /// Заметка по id.
    pub async fn get_note(&self, id: String) -> Result<Option<Note>, TensaError> {
        let id = parse_uuid("notes.get", &id)?;
        Ok(self.inner.get_note(id).await?.map(to_note))
    }

    /// Поиск (FTS5).
    pub async fn search_notes(&self, query: String, limit: u32) -> Result<Vec<Note>, TensaError> {
        let found = self.inner.search_notes(&query, limit as usize).await?;
        Ok(found.into_iter().map(to_note).collect())
    }

    /// Список заметок (свежие первыми).
    pub async fn list_notes(&self, limit: u32, offset: u32) -> Result<Vec<Note>, TensaError> {
        let listed = self
            .inner
            .list_notes(limit as usize, offset as usize)
            .await?;
        Ok(listed.into_iter().map(to_note).collect())
    }
}

/// Конвертация заметки ядра в UDL-представление.
fn to_note(note: tensa_core::Note) -> Note {
    Note {
        id: uuid_str(note.id),
        title: note.title,
        body: note.body,
        tags: note.tags,
        created_at: note.created_at,
        updated_at: note.updated_at,
    }
}
