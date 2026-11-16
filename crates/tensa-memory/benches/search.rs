//! Benchmark `search_10k` (02-testing.md): семантический поиск по архиву
//! из 10 000 фактов (SqliteArchivalMemoryStore + HashEmbedder).

use std::sync::Arc;

use criterion::{criterion_group, criterion_main, Criterion};
use tensa_memory::{
    ArchivalEntry, DefaultMemoryManager, HashEmbedder, MemoryManager, MemoryManagerConfig,
    SqliteArchivalMemoryStore, SqliteCoreMemoryStore, SqliteRecallMemoryStore,
};

/// Менеджер с архивом на 10 000 фактов.
async fn build_manager() -> DefaultMemoryManager {
    let core = SqliteCoreMemoryStore::new_in_memory().await.expect("core");
    let recall = SqliteRecallMemoryStore::new_in_memory()
        .await
        .expect("recall");
    let archival = SqliteArchivalMemoryStore::new_in_memory(Arc::new(HashEmbedder::new(384)))
        .await
        .expect("archival");
    let manager = DefaultMemoryManager::new(
        Arc::new(core),
        Arc::new(recall),
        Arc::new(archival),
        MemoryManagerConfig::default(),
        "bench-node".into(),
    )
    .await
    .expect("менеджер");

    for index in 0..10_000u32 {
        let entry = ArchivalEntry {
            id: uuid::Uuid::new_v4(),
            content: format!(
                "Факт №{index}: смета за апрель по договору {index}, сумма {index} рублей, документы и отчёты"
            ),
            embedding: Vec::new(), // эмбеддинг посчитает хранилище
            tags: vec!["сметы".into(), format!("договор-{index}")],
            source: None,
            created_at: 0,
            node_id: "bench-node".into(),
            metadata: serde_json::json!({}),
        };
        manager.archival_insert(entry).await.expect("вставка факта");
    }
    manager
}

fn bench_search_10k(c: &mut Criterion) {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");
    let manager = runtime.block_on(build_manager());

    c.bench_function("search_10k", |b| {
        b.iter(|| {
            runtime.block_on(async {
                manager
                    .archival_search("смета за апрель", 10)
                    .await
                    .expect("поиск")
            })
        })
    });
}

criterion_group!(benches, bench_search_10k);
criterion_main!(benches);
