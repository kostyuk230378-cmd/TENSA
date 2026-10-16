//! `ContactsStore` (Контракт 9): контакты поверх tensa-core.

use tensa_core::{
    ContactsStore as CoreContactsStore, NewContact as CoreNewContact, SqliteContactsStore,
};

use crate::convert::{data_root, load_core_config, parse_uuid, uuid_str};
use crate::error::TensaError;
use crate::types::{Contact, NewContact};

/// FFI-обёртка хранилища контактов (`interface ContactsStore` в UDL).
pub struct ContactsStore {
    inner: SqliteContactsStore,
}

impl ContactsStore {
    /// Открыть хранилище (база — `<root>/contacts.db`).
    pub async fn new(config_path: String) -> Result<Self, TensaError> {
        let config = load_core_config(&config_path)?;
        let store = SqliteContactsStore::new(
            &data_root(&config_path).join("contacts.db"),
            config.general.node_id.clone(),
        )
        .await?;
        Ok(Self { inner: store })
    }

    /// Создать контакт.
    pub async fn create_contact(&self, contact: NewContact) -> Result<Contact, TensaError> {
        let created = self
            .inner
            .create_contact(CoreNewContact {
                name: contact.name,
                email: contact.email,
                phone: contact.phone,
                telegram: contact.telegram,
                whatsapp: contact.whatsapp,
                notes: contact.notes,
                tags: contact.tags,
            })
            .await?;
        Ok(to_contact(created))
    }

    /// Обновить контакт (поля ядра вне UDL сохраняются).
    pub async fn update_contact(&self, contact: Contact) -> Result<Contact, TensaError> {
        let id = parse_uuid("contacts.update", &contact.id)?;
        let mut current = self.inner.get_contact(id).await?.ok_or_else(|| {
            TensaError::internal("contacts.update", format!("контакт {id} не найден"))
        })?;
        current.name = contact.name;
        current.email = contact.email;
        current.phone = contact.phone;
        current.telegram = contact.telegram;
        current.whatsapp = contact.whatsapp;
        current.notes = contact.notes;
        current.tags = contact.tags;
        let updated = self.inner.update_contact(current).await?;
        Ok(to_contact(updated))
    }

    /// Удалить контакт.
    pub async fn delete_contact(&self, id: String) -> Result<(), TensaError> {
        let id = parse_uuid("contacts.delete", &id)?;
        self.inner.delete_contact(id).await?;
        Ok(())
    }

    /// Контакт по id.
    pub async fn get_contact(&self, id: String) -> Result<Option<Contact>, TensaError> {
        let id = parse_uuid("contacts.get", &id)?;
        Ok(self.inner.get_contact(id).await?.map(to_contact))
    }

    /// Поиск по почте.
    pub async fn find_by_email(&self, email: String) -> Result<Option<Contact>, TensaError> {
        Ok(self.inner.find_by_email(&email).await?.map(to_contact))
    }

    /// Поиск по имени.
    pub async fn find_by_name(&self, name: String) -> Result<Vec<Contact>, TensaError> {
        let found = self.inner.find_by_name(&name).await?;
        Ok(found.into_iter().map(to_contact).collect())
    }

    /// Полнотекстовый поиск.
    pub async fn search_contacts(
        &self,
        query: String,
        limit: u32,
    ) -> Result<Vec<Contact>, TensaError> {
        let found = self.inner.search_contacts(&query, limit as usize).await?;
        Ok(found.into_iter().map(to_contact).collect())
    }
}

/// Конвертация контакта ядра в UDL-представление.
fn to_contact(contact: tensa_core::Contact) -> Contact {
    Contact {
        id: uuid_str(contact.id),
        name: contact.name,
        email: contact.email,
        phone: contact.phone,
        telegram: contact.telegram,
        whatsapp: contact.whatsapp,
        notes: contact.notes,
        tags: contact.tags,
        created_at: contact.created_at,
        updated_at: contact.updated_at,
    }
}
