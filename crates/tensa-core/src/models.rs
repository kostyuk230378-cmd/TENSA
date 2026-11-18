//! Shared types (tensa-core spec: Note, Contact, enums).
//! VectorClock is reused from tensa-approval (the "no code duplication" rule).

pub use tensa_approval::VectorClock;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A note (Contract 3: the `notes` table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Note {
    /// UUID (primary key).
    pub id: Uuid,
    /// Note title.
    pub title: String,
    /// Note body.
    pub body: String,
    /// Tags (JSON array in storage).
    pub tags: Vec<String>,
    /// Creation time, Unix timestamp.
    pub created_at: i64,
    /// Last update time, Unix timestamp.
    pub updated_at: i64,
    /// Node that created the note.
    pub created_on_node: String,
    /// Node that last updated the note.
    pub updated_on_node: String,
    /// Soft delete flag (Contract 3).
    pub deleted: bool,
    /// Vector clock for sync conflict resolution.
    pub vector_clock: VectorClock,
}

/// Request to create a note (Contract 12 request body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewNote {
    /// Note title.
    pub title: String,
    /// Note body.
    pub body: String,
    /// Tags.
    pub tags: Vec<String>,
}

/// A contact (Contract 3: the `contacts` table).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contact {
    /// UUID (primary key).
    pub id: Uuid,
    /// Display name.
    pub name: String,
    /// Email address, if known.
    pub email: Option<String>,
    /// Phone number, if known.
    pub phone: Option<String>,
    /// Telegram username, if known.
    pub telegram: Option<String>,
    /// WhatsApp number, if known.
    pub whatsapp: Option<String>,
    /// Free-form notes about the contact.
    pub notes: Option<String>,
    /// Tags (JSON array in storage).
    pub tags: Vec<String>,
    /// Creation time, Unix timestamp.
    pub created_at: i64,
    /// Last update time, Unix timestamp.
    pub updated_at: i64,
    /// Node that created the contact.
    pub created_on_node: String,
    /// Node that last updated the contact.
    pub updated_on_node: String,
    /// Soft delete flag (Contract 3).
    pub deleted: bool,
    /// Vector clock for sync conflict resolution.
    pub vector_clock: VectorClock,
}

/// Request to create a contact (Contract 12 request body).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewContact {
    /// Display name (required).
    pub name: String,
    /// Email address, if known.
    pub email: Option<String>,
    /// Phone number, if known.
    pub phone: Option<String>,
    /// Telegram username, if known.
    pub telegram: Option<String>,
    /// WhatsApp number, if known.
    pub whatsapp: Option<String>,
    /// Free-form notes about the contact.
    pub notes: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
}

/// UI language (Contract 8: Language).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// Russian.
    Ru,
    /// English.
    En,
}

impl Language {
    /// Lowercase code as used in config and HTTP API (`"ru"` / `"en"`).
    pub fn as_str(&self) -> &'static str {
        match self {
            Language::Ru => "ru",
            Language::En => "en",
        }
    }
}

/// Device type (Contract 8: Platform, shortened per spec).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeviceType {
    /// Desktop (Windows/Linux).
    Desktop,
    /// Android device.
    Android,
}

/// Node role in the network (Contract 8: NodeRole).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeRole {
    /// Heavy node: runs local inference.
    Heavy,
    /// Thin node: delegates inference to heavy nodes.
    Thin,
    /// Role chosen automatically from device resources.
    Auto,
}

/// Mail account (tensa.udl: `EmailAccount`).
///
/// \[derived\] Adds `email` (required by the core) and `credential` — a secret
/// stored only in secrets.toml, never in config.toml. `Debug` hides it.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct EmailAccount {
    /// Account identifier (`[email.accounts.<id>]` in config.toml).
    pub id: String,
    /// Display label.
    pub label: String,
    /// Email address (also the IMAP login).
    pub email: String,
    /// IMAP host.
    pub imap_host: String,
    /// IMAP port.
    pub imap_port: u16,
    /// SMTP host.
    pub smtp_host: String,
    /// SMTP port.
    pub smtp_port: u16,
    /// App password / token — a secret; never logged or serialized back.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential: Option<String>,
}

impl std::fmt::Debug for EmailAccount {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `credential` (a secret) is intentionally not printed.
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

/// WhatsApp Cloud API configuration (tensa.udl: `WhatsAppConfig`).
///
/// \[derived\] `credential` (the access token) is a secret; `Debug` hides it.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct WhatsAppConfig {
    /// Access token — a secret; never logged.
    pub credential: String,
    /// Phone number identifier.
    pub phone_number_id: String,
    /// WhatsApp Business Account identifier.
    pub waba_id: String,
}

impl std::fmt::Debug for WhatsAppConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // `credential` (a secret) is intentionally not printed.
        f.debug_struct("WhatsAppConfig")
            .field("phone_number_id", &self.phone_number_id)
            .field("waba_id", &self.waba_id)
            .finish_non_exhaustive()
    }
}

// --- Storage traits (Contract 3 / tensa-core spec, signatures verbatim) ---
// The spec has no separate file for them — they live next to the types.

/// Store of notes (Contract 3).
#[async_trait::async_trait]
pub trait NotesStore: Send + Sync {
    /// Create a note (server assigns id/timestamps/vector clock).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn create_note(&self, note: NewNote) -> crate::error::Result<Note>;
    /// Update a note (LWW by `updated_at` + vector clock tick).
    ///
    /// # Errors
    /// Unknown note returns [`crate::error::CoreError::NotFound`].
    async fn update_note(&self, note: Note) -> crate::error::Result<Note>;
    /// Soft-delete a note (Contract 3).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn delete_note(&self, id: Uuid) -> crate::error::Result<()>;
    /// Get a note by id; `Ok(None)` when missing or soft-deleted.
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn get_note(&self, id: Uuid) -> crate::error::Result<Option<Note>>;
    /// Full-text search (FTS5, `unicode61` — no word-form matching).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn search_notes(&self, query: &str, limit: usize) -> crate::error::Result<Vec<Note>>;
    /// List notes (limit/offset paging).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn list_notes(&self, limit: usize, offset: usize) -> crate::error::Result<Vec<Note>>;
}

/// Store of contacts (Contract 3).
#[async_trait::async_trait]
pub trait ContactsStore: Send + Sync {
    /// Create a contact (server assigns id/timestamps/vector clock).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn create_contact(&self, contact: NewContact) -> crate::error::Result<Contact>;
    /// Update a contact (LWW by `updated_at` + vector clock tick).
    ///
    /// # Errors
    /// Unknown contact returns [`crate::error::CoreError::NotFound`].
    async fn update_contact(&self, contact: Contact) -> crate::error::Result<Contact>;
    /// Soft-delete a contact (Contract 3).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn delete_contact(&self, id: Uuid) -> crate::error::Result<()>;
    /// Get a contact by id; `Ok(None)` when missing or soft-deleted.
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn get_contact(&self, id: Uuid) -> crate::error::Result<Option<Contact>>;
    /// Find a contact by exact email.
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn find_by_email(&self, email: &str) -> crate::error::Result<Option<Contact>>;
    /// Find contacts by name.
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn find_by_name(&self, name: &str) -> crate::error::Result<Vec<Contact>>;
    /// Full-text search (FTS5, `unicode61`).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn search_contacts(
        &self,
        query: &str,
        limit: usize,
    ) -> crate::error::Result<Vec<Contact>>;
    /// List contacts (limit/offset paging).
    ///
    /// # Errors
    /// Storage failure returns [`crate::error::CoreError`].
    async fn list_contacts(
        &self,
        limit: usize,
        offset: usize,
    ) -> crate::error::Result<Vec<Contact>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn sample_contact() -> Contact {
        Contact {
            id: Uuid::new_v4(),
            name: "Иван".into(),
            email: Some("ivan@example.com".into()),
            phone: Some("+7 900".into()),
            telegram: Some("@ivan".into()),
            whatsapp: None,
            notes: None,
            tags: vec!["работа".into()],
            created_at: 1,
            updated_at: 2,
            created_on_node: "a".into(),
            updated_on_node: "a".into(),
            deleted: false,
            vector_clock: VectorClock::new(),
        }
    }

    /// Контакт — serde-раундтрип без потерь полей.
    #[test]
    fn contact_serde_roundtrip() {
        let contact = sample_contact();
        let value = serde_json::to_value(&contact).expect("в JSON");
        assert_eq!(value["name"], "Иван");
        assert_eq!(value["email"], "ivan@example.com");
        assert_eq!(value["tags"][0], "работа");
        assert_eq!(value["deleted"], false);
        let restored: Contact = serde_json::from_value(value).expect("из JSON");
        assert_eq!(
            serde_json::to_value(&restored).expect("обратно"),
            serde_json::to_value(&contact).expect("оригинал")
        );
    }

    /// NewContact — десериализация тела HTTP-запроса (Контракт 12).
    #[test]
    fn new_contact_deserializes_from_body() {
        let body = serde_json::json!({
            "name": "Ольга",
            "telegram": "@olga",
            "tags": []
        });
        let new_contact: NewContact = serde_json::from_value(body).expect("тело запроса");
        assert_eq!(new_contact.name, "Ольга");
        assert_eq!(new_contact.email, None);
        assert!(new_contact.tags.is_empty());
    }

    /// Debug EmailAccount/WhatsAppConfig не печатает секреты.
    #[test]
    fn email_account_debug_hides_fields() {
        let account = EmailAccount {
            id: "personal".into(),
            label: "Личный".into(),
            email: "user@example.com".into(),
            imap_host: "imap.example.com".into(),
            imap_port: 993,
            smtp_host: "smtp.example.com".into(),
            smtp_port: 587,
            credential: Some("секрет".into()),
        };
        let debug = format!("{account:?}");
        assert!(!debug.contains("секрет"), "секрет в Debug: {debug}");
        let whatsapp = WhatsAppConfig {
            credential: "whatsapp-секрет".into(),
            phone_number_id: "123".into(),
            waba_id: "456".into(),
        };
        let debug = format!("{whatsapp:?}");
        assert!(
            !debug.contains("whatsapp-секрет"),
            "секрет в Debug: {debug}"
        );
    }

    /// EmailAccount — serde-раундтрип (Контракт 9: dictionary EmailAccount).
    #[test]
    fn email_account_serde_roundtrip() {
        let account = EmailAccount {
            id: "personal".into(),
            label: "Личный".into(),
            email: "user@example.com".into(),
            credential: None,
            imap_host: "imap.example.com".into(),
            imap_port: 993,
            smtp_host: "smtp.example.com".into(),
            smtp_port: 587,
        };
        let restored: EmailAccount =
            serde_json::from_value(serde_json::to_value(&account).expect("в JSON"))
                .expect("из JSON");
        assert_eq!(restored, account);
    }

    /// Note — serde-раундтрип (Контракт 3).
    #[test]
    fn note_serde_roundtrip() {
        let note = Note {
            id: Uuid::new_v4(),
            title: "Заметка".into(),
            body: "Текст".into(),
            tags: vec![],
            created_at: 1,
            updated_at: 2,
            created_on_node: "a".into(),
            updated_on_node: "b".into(),
            deleted: true,
            vector_clock: VectorClock::new(),
        };
        let restored: Note =
            serde_json::from_value(serde_json::to_value(&note).expect("в JSON")).expect("из JSON");
        assert_eq!(
            serde_json::to_value(&restored).expect("обратно"),
            serde_json::to_value(&note).expect("оригинал")
        );
    }

    /// Language — serde в нижний регистр (Контракт 8).
    #[test]
    fn language_serde_lowercase() {
        assert_eq!(serde_json::to_value(Language::Ru).expect("ru"), "ru");
        assert_eq!(serde_json::to_value(Language::En).expect("en"), "en");
    }
}
