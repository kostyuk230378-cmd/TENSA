//! Property-тесты vector clock (Фаза 5, 02-testing.md): merge = покомпонентный
//! максимум, tick = +1, идемпотентность merge, serde-раундтрип.

use proptest::prelude::*;
use tensa_approval::VectorClock;

/// Стратегия: часы из случайных узлов (имя → счётчик 0..5).
fn clock_strategy() -> impl Strategy<Value = (Vec<String>, Vec<u64>)> {
    proptest::collection::vec("[a-z]{1,5}", 1..6).prop_flat_map(|nodes| {
        let counters = proptest::collection::vec(0u64..5, nodes.len());
        (Just(nodes), counters)
    })
}

fn build(nodes: &[String], counters: &[u64]) -> VectorClock {
    let mut clock = VectorClock::new();
    for (node, counter) in nodes.iter().zip(counters.iter()) {
        for _ in 0..*counter {
            clock.tick(node);
        }
    }
    clock
}

proptest! {
    /// Merge = покомпонентный максимум, включая ключи, которых не было.
    #[test]
    fn merge_takes_componentwise_max(
        (a_nodes, a_counters) in clock_strategy(),
        (b_nodes, b_counters) in clock_strategy(),
    ) {
        let b = build(&b_nodes, &b_counters);

        let mut merged = build(&a_nodes, &a_counters);
        merged.merge(&b);

        // Все компоненты b покрываются с максимумом.
        for value in serde_json::to_value(&b).expect("b2").as_object().expect("map") {
            let (node, counter) = value;
            let remote = counter.as_u64().expect("counter");
            let local = serde_json::to_value(&merged)
                .expect("merged")
                .as_object()
                .expect("map")
                .get(node)
                .and_then(|value| value.as_u64())
                .unwrap_or(0);
            prop_assert!(local >= remote, "компонента {node}: {local} < {remote}");
        }

    }

    /// Merge идемпотентен: повторный merge не меняет часы.
    #[test]
    fn merge_is_idempotent(
        (a_nodes, a_counters) in clock_strategy(),
        (b_nodes, b_counters) in clock_strategy(),
    ) {
        let mut once = build(&a_nodes, &a_counters);
        let b = build(&b_nodes, &b_counters);
        once.merge(&b);
        let after_once = serde_json::to_value(&once).expect("раз");
        once.merge(&b);
        let after_twice = serde_json::to_value(&once).expect("два");
        prop_assert_eq!(after_once, after_twice);
    }

    /// Tick увеличивает счётчик узла ровно на 1.
    #[test]
    fn tick_increments_by_one(
        (nodes, counters) in clock_strategy(),
        extra in "[a-z]{1,5}",
    ) {
        let mut clock = build(&nodes, &counters);
        let before = serde_json::to_value(&clock)
            .expect("до")
            .as_object()
            .expect("map")
            .get(&extra)
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        clock.tick(&extra);
        let after = serde_json::to_value(&clock)
            .expect("после")
            .as_object()
            .expect("map")
            .get(&extra)
            .and_then(|value| value.as_u64())
            .unwrap_or(0);
        prop_assert_eq!(after, before + 1);
    }

    /// Сериализация — раундтрип без потерь.
    #[test]
    fn serde_roundtrip_preserves_clock(
        (nodes, counters) in clock_strategy(),
    ) {
        let clock = build(&nodes, &counters);
        let value = serde_json::to_value(&clock).expect("в JSON");
        let restored: VectorClock = serde_json::from_value(value).expect("из JSON");
        prop_assert_eq!(
            serde_json::to_value(&clock).expect("оригинал"),
            serde_json::to_value(&restored).expect("восстановленный")
        );
    }
}
