//! Векторные часы (Контракт 8: `tensa.common.VectorClock`).
//! Конфликты разрешаются LWW по timestamp поверх этих часов.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Отображение node_id -> счётчик событий узла.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct VectorClock(#[serde(with = "clock_map")] BTreeMap<String, u64>);

mod clock_map {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use std::collections::BTreeMap;

    #[allow(clippy::type_complexity)]
    pub fn serialize<S: Serializer>(v: &BTreeMap<String, u64>, s: S) -> Result<S::Ok, S::Error> {
        v.serialize(s)
    }

    #[allow(clippy::type_complexity)]
    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<BTreeMap<String, u64>, D::Error> {
        BTreeMap::deserialize(d)
    }
}

impl VectorClock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Собрать часы из готовой карты (декодирование proto/JSON источников).
    pub fn from_map(map: BTreeMap<String, u64>) -> Self {
        Self(map)
    }

    /// Инкремент счётчика узла (при локальном событии).
    pub fn tick(&mut self, node_id: &str) {
        *self.0.entry(node_id.to_string()).or_insert(0) += 1;
    }

    /// Слияние с чужими часами (max по каждому узлу).
    pub fn merge(&mut self, other: &VectorClock) {
        for (node, counter) in other.iter() {
            let slot = self.0.entry(node.clone()).or_insert(0);
            if *counter > *slot {
                *slot = *counter;
            }
        }
    }

    /// Часы «новее» чужих? (доминирование: все счётчики >= чужих,
    /// хотя бы один — строго больше).
    pub fn happens_after(&self, other: &VectorClock) -> bool {
        self.iter().all(|(n, c)| other.get(n) <= *c) && self.iter().any(|(n, c)| other.get(n) < *c)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &u64)> {
        self.0.iter()
    }

    pub fn get(&self, node_id: &str) -> u64 {
        self.0.get(node_id).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_and_get() {
        let mut vc = VectorClock::new();
        assert_eq!(vc.get("a"), 0);
        vc.tick("a");
        vc.tick("a");
        assert_eq!(vc.get("a"), 2);
    }

    #[test]
    fn merge_takes_max() {
        let mut a = VectorClock::new();
        a.tick("a");
        let mut b = VectorClock::new();
        b.tick("b");
        b.tick("b");
        a.merge(&b);
        assert_eq!(a.get("b"), 2);
        assert_eq!(a.get("a"), 1);
    }

    #[test]
    fn happens_after_semantics() {
        let mut a = VectorClock::new();
        a.tick("a");
        let b = a.clone();
        let mut c = VectorClock::new();
        c.tick("a");
        c.tick("b");
        assert!(!a.happens_after(&b)); // равные — не «после»
        assert!(c.happens_after(&a));
        assert!(!a.happens_after(&c));
    }

    #[test]
    fn json_roundtrip() {
        let mut vc = VectorClock::new();
        vc.tick("node-1");
        let json = serde_json::to_string(&vc).expect("serialize");
        let back: VectorClock = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.get("node-1"), 1);
    }
}
