//! Integration-тесты tensa-core (по списку ТЗ):
//! заметки/контакты CRUD+FTS5+soft delete, health, ConfigStore, запуск/остановка.

use tensa_approval::SqliteApprovalStore;
use tensa_core::*;

fn note(title: &str) -> NewNote {
    NewNote {
        title: title.into(),
        body: format!("Тело заметки {title}"),
        tags: vec!["тест".into()],
    }
}

fn contact(name: &str, email: &str) -> NewContact {
    NewContact {
        name: name.into(),
        email: Some(email.into()),
        phone: None,
        telegram: Some(format!("@{name}")),
        whatsapp: None,
        notes: Some("Заметка о контакте".into()),
        tags: vec![],
    }
}

// --- NotesStore ---

#[tokio::test]
async fn notes_crud_cycle() {
    let store = SqliteNotesStore::new_in_memory().await.expect("store");
    let created = store.create_note(note("встреча")).await.expect("create");
    assert!(!created.id.is_nil());
    assert_eq!(created.created_on_node, "local");

    let fetched = store
        .get_note(created.id)
        .await
        .expect("get")
        .expect("exists");
    assert_eq!(fetched.title, "встреча");

    let mut updated = fetched.clone();
    updated.title = "встреча обновлена".into();
    let updated = store.update_note(updated).await.expect("update");
    assert!(updated.updated_at >= created.created_at);

    store.delete_note(created.id).await.expect("delete");
    // API скрывает soft-deleted записи (deleted = 0 в запросах).
    let after = store.get_note(created.id).await.expect("get");
    assert!(after.is_none());
}

#[tokio::test]
async fn notes_fts_search() {
    let store = SqliteNotesStore::new_in_memory().await.expect("store");
    store.create_note(note("покупка кофе")).await.expect("1");
    store
        .create_note(note("отчёт за квартал"))
        .await
        .expect("2");
    let hits = store.search_notes("кофе", 10).await.expect("search");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].title, "покупка кофе");
    // Пустой запрос не должен падать.
    assert!(store.search_notes("", 10).await.is_ok());
}

#[tokio::test]
async fn notes_list_pagination() {
    let store = SqliteNotesStore::new_in_memory().await.expect("store");
    for i in 0..5 {
        store
            .create_note(note(&format!("n{i}")))
            .await
            .expect("create");
    }
    let page1 = store.list_notes(2, 0).await.expect("page1");
    let page2 = store.list_notes(2, 2).await.expect("page2");
    assert_eq!(page1.len(), 2);
    assert_eq!(page2.len(), 2);
    assert_ne!(page1[0].id, page2[0].id);
}

// --- ContactsStore ---

#[tokio::test]
async fn contacts_crud_and_lookup() {
    let store = SqliteContactsStore::new_in_memory().await.expect("store");
    let created = store
        .create_contact(contact("Иван", "ivan@example.com"))
        .await
        .expect("create");

    let by_email = store
        .find_by_email("ivan@example.com")
        .await
        .expect("by email")
        .expect("found");
    assert_eq!(by_email.id, created.id);

    let by_name = store.find_by_name("Иван").await.expect("by name");
    assert_eq!(by_name.len(), 1);

    let mut updated = created.clone();
    updated.phone = Some("+7 900 000-00-00".into());
    store.update_contact(updated).await.expect("update");

    store.delete_contact(created.id).await.expect("delete");
    // API скрывает soft-deleted записи.
    let after = store.get_contact(created.id).await.expect("get");
    assert!(after.is_none());
}

#[tokio::test]
async fn contacts_fts_search() {
    let store = SqliteContactsStore::new_in_memory().await.expect("store");
    store
        .create_contact(contact("Мария", "maria@example.com"))
        .await
        .expect("1");
    let hits = store.search_contacts("Мария", 10).await.expect("search");
    assert_eq!(hits.len(), 1);
}

#[tokio::test]
async fn contacts_missing_email_none() {
    let store = SqliteContactsStore::new_in_memory().await.expect("store");
    assert!(store
        .find_by_email("ghost@example.com")
        .await
        .expect("none")
        .is_none());
}

// --- ConfigStore ---

#[tokio::test]
async fn config_store_load_save_reload() {
    let path = std::env::temp_dir().join(format!("tensa_cs_{}.toml", uuid::Uuid::new_v4()));
    let store = FileConfigStore::new(path.clone());
    let node_id = uuid::Uuid::new_v4().to_string();
    let config = Config::default_with_node(&node_id);
    store.save(&config).await.expect("save");
    let loaded = store.load(&path).await.expect("load");
    assert_eq!(loaded.general.node_id, node_id);
    let reloaded = store.reload().await.expect("reload");
    assert_eq!(reloaded.general.node_id, node_id);
    let current = store.get().await.expect("get");
    assert_eq!(current.general.node_id, node_id);
    std::fs::remove_file(&path).ok();
}

// --- Health + CoreRuntime (запуск/остановка) ---

#[tokio::test]
async fn core_start_stop_health() {
    let config = Config::default_with_node(&uuid::Uuid::new_v4().to_string());
    let approval = SqliteApprovalStore::init_with_config(tensa_approval::ApprovalConfig::default())
        .await
        .expect("approval");
    let mut core = CoreRuntime::new(config, std::sync::Arc::new(approval));

    let health_before = core.health("test").await.expect("health");
    assert!(!health_before.healthy); // ещё не запущено

    core.start().await.expect("start");
    assert!(core.is_running());

    let health = core.health("test").await.expect("health");
    assert!(health.healthy);
    assert!(!health.degraded);
    assert!(health.components.contains_key("approval"));
    assert!(health.components["approval"].healthy);

    core.stop().await.expect("stop");
    assert!(!core.is_running());
    let after = core.health("test").await.expect("health after");
    assert!(!after.healthy);
}

// --- Новые методы Контракта 3 (одобрено 2026-10-15) ---

#[tokio::test]
async fn notes_since_and_count() {
    let store = SqliteNotesStore::new_in_memory().await.expect("store");
    let a = store.create_note(note("a")).await.expect("a");
    let b = store.create_note(note("b")).await.expect("b");

    assert_eq!(store.count_notes().await.expect("count"), 2);
    let all = store.get_notes_since(0).await.expect("since 0");
    assert_eq!(all.len(), 2);
    // Порядок по updated_at ASC (для корректной синхронизации).
    assert!(all[0].updated_at <= all[1].updated_at);

    // Watermark: ничего нового после b.updated_at.
    assert!(store
        .get_notes_since(b.updated_at)
        .await
        .expect("since b")
        .is_empty());

    // Soft delete: count уменьшается, since не отдаёт удалённую.
    store.delete_note(a.id).await.expect("delete");
    assert_eq!(store.count_notes().await.expect("count2"), 1);
    let after = store.get_notes_since(0).await.expect("since v2");
    assert_eq!(after.len(), 1);
    assert_eq!(after[0].id, b.id);
}

#[tokio::test]
async fn contacts_since_and_count() {
    let store = SqliteContactsStore::new_in_memory().await.expect("store");
    let c = store
        .create_contact(contact("Petya", "p@e.com"))
        .await
        .expect("c");
    assert_eq!(store.count_contacts().await.expect("count"), 1);
    let all = store.get_contacts_since(0).await.expect("since");
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].id, c.id);
    assert!(store
        .get_contacts_since(c.updated_at)
        .await
        .expect("none")
        .is_empty());
    store.delete_contact(c.id).await.expect("delete");
    assert_eq!(store.count_contacts().await.expect("count2"), 0);
}
