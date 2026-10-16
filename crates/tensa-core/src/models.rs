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

/// Учётная запись почты (ТЗ/UDL tensa.udl: `EmailAccount`).
/// [derived] Добавлены `email` (адрес нужен ядру) и `credential` — секрет
/// (уходит только в secrets.toml, в config.toml не пишется); `Debug` секрет не печатает.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct EmailAccount {
    pub id: String,
    pub label: String,
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

impl std::fmt::Debug for EmailAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmailAccount")
            .field("id", &self.id)
            .field("label", &self.label)
            .field("email", &self.email)
            .field("imap_host", &self.imap_host)
            .field("imap_port", &self.imap_port)
            .field("smtp_host", &self.smtp_host)
            .field("smtp_port", &self.smtp_port)
            .finish_non_exhaustive()
    }
}

/// WhatsApp Cloud API (ТЗ/UDL tensa.udl: `WhatsAppConfig`).
/// [derived] Поле-секрет — `credential` (access token); `Debug` его не печатает.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    pub credential: String,
    pub phone_number_id: String,
    pub waba_id: String,
}

impl std::fmt::Debug for WhatsAppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhatsAppConfig")
            .field("phone_number_id", &self.phone_number_id)
            .field("waba_id", &self.waba_id)
            .finish_non_exhaustive()
    }
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
