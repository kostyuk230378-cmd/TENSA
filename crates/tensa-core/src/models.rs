//! Общие типы (ТЗ tensa-core: Note, Contact, перечисления).
//! VectorClock переиспользуется из tensa-approval (правило «не дублировать код»).

pub use tensa_approval::VectorClock;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Заметка (Контракт 3: таблица notes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    pub id: Uuid,
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub created_on_node: String,
    pub updated_on_node: String,
    pub deleted: bool,
    pub vector_clock: VectorClock,
}

/// Заявка на создание заметки.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewNote {
    pub title: String,
    pub body: String,
    pub tags: Vec<String>,
}

/// Контакт (Контракт 3: таблица contacts).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    pub id: Uuid,
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub telegram: Option<String>,
    pub whatsapp: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub created_on_node: String,
    pub updated_on_node: String,
    pub deleted: bool,
    pub vector_clock: VectorClock,
}

/// Заявка на создание контакта.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewContact {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub telegram: Option<String>,
    pub whatsapp: Option<String>,
    pub notes: Option<String>,
    pub tags: Vec<String>,
}

/// Язык интерфейса (Контракт 8: Language).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Ru,
    En,
}

impl Language {
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Ru => "ru",
            Language::En => "en",
        }
    }
}

/// Тип устройства (Контракт 8: Platform, сокращённо по ТЗ).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    Desktop,
    Android,
}

/// Роль узла (Контракт 8: NodeRole).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    Heavy,
    Thin,
    Auto,
}

// --- Трейты хранилищ (Контракт 3 / ТЗ tensa-core, сигнатуры дословно) ---
// В структуре ТЗ для них нет отдельного файла — живут рядом с типами.

/// Хранилище заметок.
#[async_trait::async_trait]
pub trait NotesStore: Send + Sync {
    async fn create_note(&self, note: NewNote) -> crate::error::Result<Note>;
    async fn update_note(&self, note: Note) -> crate::error::Result<Note>;
    async fn delete_note(&self, id: Uuid) -> crate::error::Result<()>;
    async fn get_note(&self, id: Uuid) -> crate::error::Result<Option<Note>>;
    async fn search_notes(&self, query: &str, limit: usize) -> crate::error::Result<Vec<Note>>;
    async fn list_notes(&self, limit: usize, offset: usize) -> crate::error::Result<Vec<Note>>;
}

/// Хранилище контактов.
#[async_trait::async_trait]
pub trait ContactsStore: Send + Sync {
    async fn create_contact(&self, contact: NewContact) -> crate::error::Result<Contact>;
    async fn update_contact(&self, contact: Contact) -> crate::error::Result<Contact>;
    async fn delete_contact(&self, id: Uuid) -> crate::error::Result<()>;
    async fn get_contact(&self, id: Uuid) -> crate::error::Result<Option<Contact>>;
    async fn find_by_email(&self, email: &str) -> crate::error::Result<Option<Contact>>;
    async fn find_by_name(&self, name: &str) -> crate::error::Result<Vec<Contact>>;
    async fn search_contacts(
        &self,
        query: &str,
        limit: usize,
    ) -> crate::error::Result<Vec<Contact>>;
    async fn list_contacts(
        &self,
        limit: usize,
        offset: usize,
    ) -> crate::error::Result<Vec<Contact>>;
}
