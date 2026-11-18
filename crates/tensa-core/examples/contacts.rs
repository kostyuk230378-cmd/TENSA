//! Пример: заметки и контакты через in-memory хранилища tensa-core
//! (Контракт 3). Запуск: cargo run -p tensa-core --example contacts

use tensa_core::{
    ContactsStore, NewContact, NewNote, NotesStore, SqliteContactsStore, SqliteNotesStore,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // In-memory БД (для файлов — SqliteNotesStore::new(&path, node_id)).
    let notes = SqliteNotesStore::new_in_memory().await?;
    let contacts = SqliteContactsStore::new_in_memory().await?;

    // Заметка: создать → найти → удалить (soft delete).
    let note = notes
        .create_note(NewNote {
            title: "Смета за апрель".into(),
            body: "Черновик сметы, приложить акты.".into(),
            tags: vec!["сметы".into()],
        })
        .await?;
    println!("создана заметка {}", note.id);

    let found = notes.search_notes("смета", 10).await?;
    println!("поиск «смета»: {} результатов", found.len());

    notes.delete_note(note.id).await?;
    println!("заметка {} удалена", note.id);

    // Контакт: создать → найти по имени.
    let contact = contacts
        .create_contact(NewContact {
            name: "Иван Петров".into(),
            email: Some("ivan@example.com".into()),
            phone: None,
            telegram: Some("@ivan".into()),
            whatsapp: None,
            notes: None,
            tags: vec!["работа".into()],
        })
        .await?;
    println!("создан контакт {}", contact.id);

    let by_name = contacts.find_by_name("Иван Петров").await?;
    println!("по имени найдено: {}", by_name.len());

    Ok(())
}
