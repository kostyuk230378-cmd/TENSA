//! Tensa Desktop — отдельное окно секретаря (WebView2), а не вкладка браузера.
//!
//! Запуск:
//! 1. если `127.0.0.1:8765` не слушает — стартует `tensa-server.exe` из
//!    каталога установки (данные — `%USERPROFILE%\.tensa`), ждёт готовности;
//! 2. открывает окно WebView2 на `http://127.0.0.1:8765/app`;
//! 3. при закрытии окна останавливает сервер, если запускал его сам.
//!
//! Сборка под другие ОС — заглушка (CI собирает без GUI-зависимостей).

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        let _ = log_error(&error.to_string());
    }
}

#[cfg(not(windows))]
fn main() {
    println!("tensa-desktop: окно WebView2 доступно только на Windows");
}

/// Записать ошибку запуска в `~/.tensa/desktop.log` (окно может не открыться).
#[cfg(windows)]
fn log_error(message: &str) -> std::io::Result<()> {
    use std::io::Write;
    let root = std::env::var("USERPROFILE")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("."));
    let dir = root.join(".tensa");
    std::fs::create_dir_all(&dir)?;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("desktop.log"))?;
    writeln!(file, "{message}")
}

/// Запустить окно и (при необходимости) ядро.
#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Child;
    use std::time::{Duration, Instant};

    use tao::dpi::LogicalSize;
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoop};
    use tao::window::WindowBuilder;

    let address = "127.0.0.1:8765";
    let url = "http://127.0.0.1:8765/app";

    let mut child: Option<Child> = None;
    if !port_open(address) {
        child = Some(start_server()?);
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if port_open(address) {
                break;
            }
            std::thread::sleep(Duration::from_millis(300));
        }
    }

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("Tensa")
        .with_inner_size(LogicalSize::new(460.0, 820.0))
        .with_min_inner_size(LogicalSize::new(380.0, 600.0))
        .build(&event_loop)?;
    let _webview = wry::WebViewBuilder::new().with_url(url).build(&window)?;

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            if let Some(mut process) = child.take() {
                let _ = process.kill();
            }
            *control_flow = ControlFlow::Exit;
        }
    });
}

/// Проверить, слушает ли кто-то порт.
#[cfg(windows)]
fn port_open(address: &str) -> bool {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;
    match address.parse::<SocketAddr>() {
        Ok(addr) => TcpStream::connect_timeout(&addr, Duration::from_millis(400)).is_ok(),
        Err(_) => false,
    }
}

/// Запустить ядро из каталога установки (тот же, что у этого exe).
#[cfg(windows)]
fn start_server() -> std::io::Result<std::process::Child> {
    use std::path::PathBuf;
    use std::process::{Command, Stdio};

    let exe_dir = std::env::current_exe()?
        .parent()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    let server = exe_dir.join("tensa-server.exe");
    let config = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".tensa")
        .join("config.toml");

    // Логи ядра — в ~/.tensa/logs/server.log (диагностика падений и индексации).
    let mut command = Command::new(server);
    command
        .arg(config)
        .current_dir(&exe_dir)
        .env("TENSA_BIND", "0.0.0.0:8765");
    let logs_dir = std::env::var("USERPROFILE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".tensa")
        .join("logs");
    if std::fs::create_dir_all(&logs_dir).is_ok() {
        let log_path = logs_dir.join("server.log");
        if let Ok(file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
        {
            if let Ok(clone) = file.try_clone() {
                command.stdout(Stdio::from(file));
                command.stderr(Stdio::from(clone));
            }
        }
    }
    command.spawn()
}
