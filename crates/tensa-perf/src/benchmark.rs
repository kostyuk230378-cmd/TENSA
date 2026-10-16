//! Бенчмарк устройства (ТЗ tensa-perf: memory bandwidth, disk read/write,
//! CPU score; кэш `~/.tensa/benchmark.json`).

use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::Instant;

use tensa_core::paths::TensaPaths;

use crate::error::{PerfError, Result};
use crate::models::BenchmarkResult;

/// Размер блока измерения памяти, байт (64 МиБ).
const MEMORY_BLOCK_BYTES: usize = 64 * 1024 * 1024;
/// Размер файла для измерения диска, байт (32 МиБ).
const DISK_BLOCK_BYTES: usize = 32 * 1024 * 1024;
/// Число итераций CPU-нагрузки.
const CPU_ITERATIONS: u64 = 5_000_000;
/// Имя временного файла дискового бенчмарка.
const DISK_TMP_FILE: &str = "tensa-bench.tmp";

/// Выполнить полный бенчмарк (память, диск, CPU).
pub fn run(disk_dir: &Path) -> Result<BenchmarkResult> {
    let memory_bandwidth_mbps = measure_memory_bandwidth_mbps();
    let (disk_read_mbps, disk_write_mbps) = measure_disk_mbps(disk_dir)?;
    let cpu_score = measure_cpu_score();

    Ok(BenchmarkResult {
        memory_bandwidth_mbps,
        disk_read_mbps,
        disk_write_mbps,
        cpu_score,
        timestamp: chrono::Utc::now().timestamp(),
    })
}

/// Пропускная способность памяти, МиБ/с (запись + чтение блока).
pub fn measure_memory_bandwidth_mbps() -> f64 {
    let mut buffer = vec![0u8; MEMORY_BLOCK_BYTES];

    let start = Instant::now();
    for byte in buffer.iter_mut() {
        *byte = 0xA5;
    }
    let mut checksum: u64 = 0;
    for chunk in buffer.as_chunks::<8>().0 {
        checksum = checksum.wrapping_add(u64::from_ne_bytes(*chunk));
    }
    let elapsed = start.elapsed().as_secs_f64().max(1e-9);
    std::hint::black_box(checksum);

    let mib = MEMORY_BLOCK_BYTES as f64 / (1024.0 * 1024.0);
    (mib * 2.0) / elapsed
}

/// CPU-скор: тысяч итераций фиксированной нагрузки в секунду.
pub fn measure_cpu_score() -> u32 {
    let start = Instant::now();
    let mut accumulator: u64 = 1;
    for step in 1..=CPU_ITERATIONS {
        accumulator = accumulator.wrapping_mul(31).wrapping_add(step);
    }
    let elapsed = start.elapsed().as_secs_f64().max(1e-6);
    std::hint::black_box(accumulator);

    let score = (CPU_ITERATIONS as f64 / elapsed / 1000.0).min(f64::from(u32::MAX)) as u32;
    score.max(1)
}

/// Скорость диска (чтение, запись), МиБ/с.
pub fn measure_disk_mbps(dir: &Path) -> Result<(f64, f64)> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(DISK_TMP_FILE);
    let payload = vec![0x5Au8; DISK_BLOCK_BYTES];

    let start = Instant::now();
    {
        let file = std::fs::File::create(&path).map_err(|error| {
            PerfError::Benchmark(format!("не создать {}: {error}", path.display()))
        })?;
        let mut writer = BufWriter::new(file);
        writer.write_all(&payload)?;
        writer.flush()?;
        writer.get_ref().sync_all()?;
    }
    let write_seconds = start.elapsed().as_secs_f64().max(1e-9);

    let start = Instant::now();
    let mut readback: Vec<u8> = Vec::with_capacity(DISK_BLOCK_BYTES);
    {
        let mut file = std::fs::File::open(&path)?;
        file.read_to_end(&mut readback)?;
    }
    let read_seconds = start.elapsed().as_secs_f64().max(1e-9);
    std::hint::black_box(readback.len());
    let _ = std::fs::remove_file(&path);

    let mib = DISK_BLOCK_BYTES as f64 / (1024.0 * 1024.0);
    Ok((mib / read_seconds, mib / write_seconds))
}

/// Путь кэша бенчмарка: `~/.tensa/benchmark.json` (ТЗ).
pub fn cache_path(paths: &TensaPaths) -> PathBuf {
    paths.root().join("benchmark.json")
}

/// Сохранить результат бенчмарка в кэш.
pub fn save_cached(paths: &TensaPaths, result: &BenchmarkResult) -> Result<()> {
    let path = cache_path(paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(result)?;
    std::fs::write(path, json)?;
    Ok(())
}

/// Загрузить кэш бенчмарка (`None` — кэша нет).
pub fn load_cached(paths: &TensaPaths) -> Result<Option<BenchmarkResult>> {
    let path = cache_path(paths);
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let parsed = serde_json::from_str(&raw)?;
    Ok(Some(parsed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_bandwidth_is_positive() {
        assert!(measure_memory_bandwidth_mbps() > 0.0);
    }

    #[test]
    fn cpu_score_is_positive() {
        assert!(measure_cpu_score() >= 1);
    }

    #[test]
    fn cache_roundtrip() {
        let root = std::env::temp_dir().join(format!("tensa-perf-bench-{}", std::process::id()));
        let paths = TensaPaths::with_root(root.clone());
        assert!(load_cached(&paths).expect("пустой кэш").is_none());

        let result = BenchmarkResult {
            memory_bandwidth_mbps: 1234.5,
            disk_read_mbps: 100.0,
            disk_write_mbps: 80.0,
            cpu_score: 42,
            timestamp: 1_700_000_000,
        };
        save_cached(&paths, &result).expect("сохранение кэша");
        let loaded = load_cached(&paths).expect("чтение кэша");
        assert_eq!(loaded, Some(result));

        let _ = std::fs::remove_dir_all(root);
    }
}
