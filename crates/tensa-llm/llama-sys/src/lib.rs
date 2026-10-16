//! # tensa-llama-sys — уровень 1 (ADR-003)
//!
//! Raw FFI-биндинги llama.cpp: генерация через bindgen из заголовков
//! закреплённой версии (`v0.7.0`), тонкий слой, все вызовы unsafe,
//! изоляция — «ни один unsafe-вызов не выходит за пределы семейства
//! tensa-llama-*».
//!
//! **Статус:**
//! - без фичи `native` крейт компилируется пустым (workspace остаётся
//!   зелёным без libclang и сети), `tensa-llama` честно возвращает
//!   `EngineUnavailable`;
//! - с фичей `native` (TASK-ENV-001) собирается статическая llama.cpp
//!   v0.7.0 из `vendor/llama.cpp` и генерируются биндинги — см. `build.rs`
//!   и README крейта.

/// Уровень 1 собран с нативными биндингами (фича `native`)?
pub const NATIVE_READY: bool = cfg!(feature = "native");

#[cfg(feature = "native")]
#[allow(
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    dead_code,
    clippy::all
)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

#[cfg(feature = "native")]
pub use bindings::*;

/// Версия крейта.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg(all(test, feature = "native"))]
mod native_link_tests {
    //! Проверка, что статические библиотеки llama.cpp реально линкуются
    //! и символы доступны во время выполнения.
    use super::*;

    #[test]
    fn backend_init_and_default_params_link() {
        // SAFETY: инициализация бэкенда llama.cpp и чтение дефолтных
        // параметров модели — без внешнего состояния, освобождаем бэкенд.
        unsafe {
            llama_backend_init();
            let params = llama_model_default_params();
            std::hint::black_box(&params);
            llama_backend_free();
        }
    }

    #[test]
    fn sampler_symbols_link() {
        // SAFETY: создание/уничтожение сеплера llama.cpp — изолированные вызовы.
        unsafe {
            let sampler = llama_sampler_init_greedy();
            assert!(!sampler.is_null());
            llama_sampler_free(sampler);
        }
    }
}
