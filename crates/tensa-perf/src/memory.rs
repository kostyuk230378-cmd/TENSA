//! Сбор метрик памяти и дисков (sysinfo; ТЗ tensa-perf).

use sysinfo::{Disks, System};

/// Снимок оперативной памяти.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySnapshot {
    /// Всего RAM, МБ.
    pub total_mb: u64,
    /// Доступно RAM, МБ.
    pub available_mb: u64,
}

/// Снимок дисков.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskSnapshot {
    /// Всего места, ГБ.
    pub total_gb: u64,
    /// Доступно места, ГБ.
    pub available_gb: u64,
}

/// Собрать снимок RAM.
pub fn snapshot() -> MemorySnapshot {
    let mut system = System::new();
    system.refresh_memory();
    MemorySnapshot {
        total_mb: system.total_memory() / (1024 * 1024),
        available_mb: system.available_memory() / (1024 * 1024),
    }
}

/// Собрать снимок дисков.
/// [derived] Суммарный объём несъёмных дисков (ТЗ не уточняет, какие диски
/// учитывать: `disk_total_gb`/`disk_available_gb`).
pub fn disk_snapshot() -> DiskSnapshot {
    const GB: u64 = 1024 * 1024 * 1024;

    let disks = Disks::new_with_refreshed_list();
    let mut total: u64 = 0;
    let mut available: u64 = 0;
    for disk in disks.list() {
        if disk.is_removable() || disk.total_space() == 0 {
            continue;
        }
        total = total.saturating_add(disk.total_space());
        available = available.saturating_add(disk.available_space());
    }

    DiskSnapshot {
        total_gb: total / GB,
        available_gb: available / GB,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_is_sane() {
        let memory = snapshot();
        assert!(memory.total_mb > 0);
        assert!(memory.available_mb <= memory.total_mb);
    }

    #[test]
    fn disk_is_sane() {
        let disk = disk_snapshot();
        if disk.total_gb > 0 {
            assert!(disk.available_gb <= disk.total_gb);
        }
    }
}
