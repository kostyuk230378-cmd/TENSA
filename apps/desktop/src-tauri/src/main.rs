//! Бинарь Tensa Desktop (Tauri, Контракт 10): оконная оболочка IPC-команд.
//! Ошибки запуска — значением, без паник.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Err(error) = tensa_desktop_tauri::run() {
        eprintln!("ошибка: {error}");
        std::process::exit(1);
    }
}
