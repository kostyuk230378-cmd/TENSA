//! Tensa Desktop — плавающий зелёный виджет (WebView2), а не окно браузера.
//!
//! Окно: прозрачное, без рамок, поверх остальных, не в панели задач.
//! Свёрнуто — «пилюля» (микрофон · T · точки) с лампочкой уведомлений;
//! разворачивается в мини-панель (чат, одобрения, настройки) по IPC.
//! Ядро поднимается при запуске и останавливается при выходе.

#![cfg_attr(windows, windows_subsystem = "windows")]

#[cfg(windows)]
fn main() {
    if let Err(error) = run() {
        let _ = log_error(&error.to_string());
    }
}

#[cfg(not(windows))]
fn main() {
    println!("tensa-desktop: виджет доступен только на Windows (WebView2)");
}

/// Записать ошибку запуска в `~/.tensa/desktop.log`.
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

/// Команды из виджета.
#[cfg(windows)]
#[derive(Debug, Clone, Copy)]
enum Command {
    /// Начать перетаскивание окна.
    Drag,
    /// Развернуть в мини-панель.
    Expand,
    /// Свернуть в пилюлю.
    Collapse,
    /// Выход (гасит ядро).
    Quit,
}

/// Запустить виджет и (при необходимости) ядро.
#[cfg(windows)]
fn run() -> Result<(), Box<dyn std::error::Error>> {
    use std::process::Child;
    use std::time::{Duration, Instant};

    use tao::dpi::{LogicalPosition, LogicalSize};
    use tao::event::{Event, WindowEvent};
    use tao::event_loop::{ControlFlow, EventLoopBuilder};
    use tao::platform::windows::WindowBuilderExtWindows;
    use tao::window::WindowBuilder;

    let address = "127.0.0.1:8765";
    // Отладочный запуск сразу развёрнутым (для скриншотов).
    let start_expanded = std::env::var("TENSA_WIDGET_EXPANDED").as_deref() == Ok("1");
    // Cache-busting: WebView2 иначе держит старый widget.html между запусками.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    let url = if start_expanded {
        format!("http://127.0.0.1:8765/widget?expanded=1&v={stamp}")
    } else {
        format!("http://127.0.0.1:8765/widget?v={stamp}")
    };
    let (start_width, start_height) = if start_expanded {
        (440.0, 700.0)
    } else {
        (348.0, 150.0)
    };

    let mut child: Option<Child> = None;
    if !port_open(address) {
        child = Some(start_server()?);
        let deadline = Instant::now() + Duration::from_secs(60);
        while Instant::now() < deadline {
            if port_open(address) {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }
    }

    let mut builder = EventLoopBuilder::<Command>::with_user_event();
    let event_loop = builder.build();
    let proxy = event_loop.create_proxy();

    let window = WindowBuilder::new()
        .with_title("Tensa")
        .with_transparent(true)
        .with_decorations(false)
        .with_always_on_top(true)
        .with_resizable(false)
        .with_skip_taskbar(true)
        .with_inner_size(LogicalSize::new(start_width, start_height))
        .build(&event_loop)?;

    // Угол экрана: справа снизу, как в макете.
    if let Some(monitor) = window.current_monitor() {
        let size = monitor.size();
        let scale = monitor.scale_factor();
        let width = 348.0 * scale;
        let height = 150.0 * scale;
        let x = size.width as i32 - width as i32 - 40;
        let y = size.height as i32 - height as i32 - 80;
        let _ = window.set_outer_position(tao::dpi::PhysicalPosition::new(x, y));
    }

    let webview = wry::WebViewBuilder::new()
        .with_url(url)
        .with_transparent(true)
        .with_ipc_handler(move |request| {
            let message = request.body().clone();
            let command = match message.as_str() {
                "drag" => Some(Command::Drag),
                "expand" => Some(Command::Expand),
                "collapse" => Some(Command::Collapse),
                "quit" => Some(Command::Quit),
                _ => None,
            };
            if let Some(command) = command {
                let _ = proxy.send_event(command);
            }
        })
        .build(&window)?;

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(Command::Drag) => {
                let _ = window.drag_window();
            }
            Event::UserEvent(Command::Expand) => {
                resize_anchored(&window, 440.0, 700.0);
            }
            Event::UserEvent(Command::Collapse) => {
                resize_anchored(&window, 348.0, 150.0);
            }
            Event::UserEvent(Command::Quit) => {
                if let Some(mut process) = child.take() {
                    let _ = process.kill();
                }
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Some(mut process) = child.take() {
                    let _ = process.kill();
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
        // Держим webview живым до выхода.
        let _ = &webview;
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

/// Запустить ядро из каталога установки (лог — в `~/.tensa/logs/server.log`).
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

/// Изменить размер окна, сохраняя нижний правый угол на месте.
#[cfg(windows)]
fn resize_anchored(window: &tao::window::Window, width: f64, height: f64) {
    use tao::dpi::{LogicalSize, PhysicalPosition};
    let scale = window.scale_factor();
    if let Ok(position) = window.outer_position() {
        let size = window.outer_size();
        let new_width = (width * scale) as i32;
        let new_height = (height * scale) as i32;
        let dx = new_width - size.width as i32;
        let dy = new_height - size.height as i32;
        let _ = window.set_outer_position(PhysicalPosition::new(position.x - dx, position.y - dy));
    }
    let _ = window.set_inner_size(LogicalSize::new(width, height));
}
