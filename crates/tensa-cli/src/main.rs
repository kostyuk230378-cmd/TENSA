//! Бинарь `tensa` (ТЗ tensa-cli): разбор аргументов и запуск команды.
//!
//! Логи — в stderr и только по `-v` (`--verbose`); секреты не логируются.

use clap::Parser;

use tensa_cli::cli::Cli;
use tensa_cli::commands;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if cli.verbose {
        init_tracing();
    }
    match commands::dispatch(cli).await {
        Ok(()) => {}
        Err(error) => {
            eprintln!("ошибка: {error}");
            std::process::exit(error.exit_code());
        }
    }
}

/// Включить debug-логи (уровень можно переопределить через `RUST_LOG`).
fn init_tracing() {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("tensa=debug,info"));
    // Повторная инициализация в тестах не критична — не паникуем.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
