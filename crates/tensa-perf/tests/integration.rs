//! Интеграционные тесты tensa-perf (ТЗ: профилирование, бенчмарк,
//! определение роли, мониторинг, ошибки).

use std::time::Duration;

use tensa_core::paths::TensaPaths;
use tensa_perf::{benchmark, DefaultPerfProfiler, PerfConfig, PerfProfiler};

fn temp_paths() -> TensaPaths {
    let root = std::env::temp_dir().join(format!("tensa-perf-it-{}", uuid::Uuid::new_v4()));
    TensaPaths::with_root(root)
}

fn test_config() -> PerfConfig {
    PerfConfig {
        profile_interval_seconds: 1,
        ..PerfConfig::default()
    }
}

#[tokio::test]
async fn profile_is_sane() {
    let profiler =
        DefaultPerfProfiler::with_paths(test_config(), temp_paths()).expect("профилировщик");
    let profile = profiler.profile().await.expect("профиль");

    assert!(profile.ram_total_mb > 0);
    assert!(profile.ram_available_mb <= profile.ram_total_mb);
    assert!(profile.cpu_cores >= 1);
    assert!(profile.cpu_usage_percent >= 0.0);
    assert!(!profile.os.is_empty());
    assert!(!profile.arch.is_empty());
    // GPU/NPU пока не детектируются — честный None до TASK-ENV-001.
    assert!(profile.gpu.is_none());
    assert!(profile.npu.is_none());
}

#[tokio::test]
async fn benchmark_and_cache_roundtrip() {
    let paths = temp_paths();
    let profiler =
        DefaultPerfProfiler::with_paths(test_config(), paths.clone()).expect("профилировщик");
    let result = profiler.benchmark().await.expect("бенчмарк");

    assert!(result.memory_bandwidth_mbps > 0.0);
    assert!(result.disk_read_mbps > 0.0);
    assert!(result.disk_write_mbps > 0.0);
    assert!(result.cpu_score >= 1);
    assert!(result.timestamp > 0);

    let cached = benchmark::load_cached(&paths)
        .expect("чтение кэша")
        .expect("результат в кэше");

    // Сравнение по полям: целые — точно, float — с допуском: текстовый
    // roundtrip f64 (JSON) не обязан быть бит-в-бит идентичным
    // (на CI зафиксировано расхождение ~2 ULP в последнем разряде).
    assert_eq!(cached.cpu_score, result.cpu_score);
    assert_eq!(cached.timestamp, result.timestamp);
    for (cached_value, result_value) in [
        (cached.memory_bandwidth_mbps, result.memory_bandwidth_mbps),
        (cached.disk_read_mbps, result.disk_read_mbps),
        (cached.disk_write_mbps, result.disk_write_mbps),
    ] {
        let diff = (cached_value - result_value).abs();
        let scale = result_value.abs().max(1.0);
        assert!(
            diff / scale < 1e-9,
            "roundtrip-расхождение: {cached_value} vs {result_value}"
        );
    }

    let _ = std::fs::remove_dir_all(paths.root());
}

#[tokio::test]
async fn role_is_decided_from_real_profile() {
    let profiler =
        DefaultPerfProfiler::with_paths(test_config(), temp_paths()).expect("профилировщик");
    let profile = profiler.profile().await.expect("профиль");
    let decision = profiler.decide_role(&profile).await.expect("решение");

    assert!(matches!(
        decision.role,
        tensa_perf::NodeRole::Heavy | tensa_perf::NodeRole::Thin
    ));
    assert!(!decision.reason.is_empty());
    if matches!(decision.role, tensa_perf::NodeRole::Heavy) {
        assert!(decision.can_run_llm);
    } else {
        assert!(!decision.can_run_llm);
    }
}

#[tokio::test]
async fn monitoring_lifecycle() {
    let profiler =
        DefaultPerfProfiler::with_paths(test_config(), temp_paths()).expect("профилировщик");

    profiler
        .start_monitoring()
        .await
        .expect("старт мониторинга");
    assert!(
        profiler.start_monitoring().await.is_err(),
        "повторный старт — ошибка"
    );

    tokio::time::sleep(Duration::from_millis(1400)).await;
    let profile = profiler
        .current_profile()
        .await
        .expect("профиль из мониторинга");
    assert!(profile.ram_total_mb > 0);

    profiler.stop_monitoring().await.expect("стоп мониторинга");
    assert!(
        profiler.stop_monitoring().await.is_err(),
        "повторный стоп — ошибка"
    );
}

#[tokio::test]
async fn benchmark_fails_on_bad_disk_dir() {
    // Путь указывает на файл, а не на каталог: бенчмарк диска — ошибка.
    let file_path = std::env::temp_dir().join(format!("tensa-perf-file-{}", uuid::Uuid::new_v4()));
    std::fs::write(&file_path, b"not a dir").expect("подготовка файла");

    let paths = TensaPaths::with_root(file_path.clone());
    let profiler = DefaultPerfProfiler::with_paths(test_config(), paths).expect("профилировщик");
    assert!(profiler.benchmark().await.is_err());

    let _ = std::fs::remove_file(file_path);
}

#[test]
fn unsupported_sources_are_honest() {
    assert!(!tensa_perf::gpu::detection_available());
    assert!(tensa_perf::gpu::detect().is_none());
    assert!(!tensa_perf::npu::detection_available());
    assert!(tensa_perf::npu::detect().is_none());
    assert_eq!(
        tensa_perf::battery::supported(),
        cfg!(any(windows, target_os = "linux", target_os = "android"))
    );
}
