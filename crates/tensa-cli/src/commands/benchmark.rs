//! `tensa benchmark` — бенчмарк устройства (ТЗ tensa-perf).

use std::time::Duration;

use indicatif::ProgressBar;

use crate::cli::BenchmarkArgs;
use crate::commands::{ts, Ctx};
use crate::error::Result;

/// Выполнить `benchmark`.
pub async fn run(ctx: &Ctx, args: &BenchmarkArgs) -> Result<()> {
    let dir = args.dir.clone().unwrap_or_else(|| ctx.paths.root());

    // Спиннер — только для интерактивного вывода (не при --json/--quiet).
    let spinner = if ctx.output.quiet() || ctx.output.json_mode() {
        None
    } else {
        let bar = ProgressBar::new_spinner();
        bar.set_message(format!("бенчмарк в {}…", dir.display()));
        bar.enable_steady_tick(Duration::from_millis(120));
        Some(bar)
    };

    let result = tensa_perf::benchmark::run(&dir)?;

    if let Some(bar) = spinner {
        bar.finish_and_clear();
    }

    if ctx.output.json_mode() {
        return ctx.output.json(&result);
    }

    ctx.output.success("бенчмарк завершён");
    ctx.output.table(
        &["метрика", "значение"],
        &[
            vec![
                "память, МБ/с".to_string(),
                format!("{:.1}", result.memory_bandwidth_mbps),
            ],
            vec![
                "диск: чтение, МБ/с".to_string(),
                format!("{:.1}", result.disk_read_mbps),
            ],
            vec![
                "диск: запись, МБ/с".to_string(),
                format!("{:.1}", result.disk_write_mbps),
            ],
            vec!["CPU-балл".to_string(), result.cpu_score.to_string()],
            vec!["время".to_string(), ts(result.timestamp)],
        ],
    );
    Ok(())
}
