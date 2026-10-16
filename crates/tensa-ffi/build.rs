//! Генерация uniffi-scaffolding из UDL (Контракт 9).
//!
//! Источник истины FFI-поверхности — `teh_doc/contracts/ffi/tensa.udl`.
//! Результат: `$OUT_DIR/tensa.uniffi.rs`, подключается в `src/lib.rs`
//! через `uniffi::include_scaffolding!("tensa")`.
//!
//! UDL лежит вне крейта (`teh_doc/`), поэтому авто-режим
//! `generate_scaffolding` не находит крейт — используем
//! `generate_scaffolding_for_crate` с именем пакета (`tensa_ffi`):
//! так uniffi формирует символы и базовый путь для нашей библиотеки.

use std::path::PathBuf;

fn main() {
    let udl = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("teh_doc")
        .join("contracts")
        .join("ffi")
        .join("tensa.udl");

    // `generate_scaffolding_for_crate` сам печатает `rerun-if-changed` для UDL.
    if let Err(error) =
        uniffi::generate_scaffolding_for_crate(udl.to_string_lossy().as_ref(), "tensa_ffi")
    {
        eprintln!(
            "tensa-ffi: не удалось сгенерировать scaffolding из {}: {error:#}",
            udl.display()
        );
        std::process::exit(1);
    }
}
