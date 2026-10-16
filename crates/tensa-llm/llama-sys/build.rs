//! Сборка нативного слоя (ADR-003, уровень 1; TASK-ENV-001).
//!
//! Без фичи `native` крейт компилируется пустым (FFI-деклараций нет), а
//! `tensa-llama` честно сообщает `EngineUnavailable`.
//!
//! С фичей `native`:
//! 1) источник — закреплённый тег llama.cpp ([`LLAMA_CPP_TAG`]) в
//!    `vendor/llama.cpp`, либо каталог из `LLAMA_CPP_DIR`;
//! 2) cmake-сборка статических библиотек (на MSVC cmake-rs сам выбирает
//!    генератор Visual Studio и `-A x64`; бэкенды GPU/OpenCL выключены);
//! 3) bindgen-биндинги из `include/llama.h` — нужен libclang, путь в
//!    `LIBCLANG_PATH` (см. README крейта).

#[cfg(feature = "native")]
use std::path::{Path, PathBuf};

/// Закреплённая версия llama.cpp (TASK-ENV-001).
#[cfg(feature = "native")]
const LLAMA_CPP_TAG: &str = "v0.7.0";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=LLAMA_CPP_DIR");
    println!("cargo:rerun-if-env-changed=LIBCLANG_PATH");

    // Без фичи `native` крейт компилируется пустым (см. доккомментарий крейта).
    #[cfg(feature = "native")]
    {
        let source = native_source_dir();
        println!(
            "cargo:rerun-if-changed={}",
            source.join("CMakeLists.txt").display()
        );
        build_native(&source);
        generate_bindings(&source);
    }
}

/// Каталог исходников llama.cpp: env `LLAMA_CPP_DIR` или `vendor/llama.cpp`.
#[cfg(feature = "native")]
fn native_source_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("LLAMA_CPP_DIR") {
        let path = PathBuf::from(dir);
        assert!(
            path.join("CMakeLists.txt").is_file(),
            "LLAMA_CPP_DIR={}: нет CMakeLists.txt",
            path.display()
        );
        return path;
    }

    let vendor = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("vendor")
        .join("llama.cpp");
    assert!(
        vendor.join("CMakeLists.txt").is_file(),
        "нет исходников llama.cpp: выполните `git clone --depth 1 --branch {LLAMA_CPP_TAG} \
         https://github.com/ggml-org/llama.cpp vendor/llama.cpp` или задайте LLAMA_CPP_DIR"
    );
    vendor
}

/// Собрать llama.cpp статически и сообщить линковщику Rust пути и библиотеки.
#[cfg(feature = "native")]
fn build_native(source: &Path) {
    let install_dir = cmake::Config::new(source)
        .define("BUILD_SHARED_LIBS", "OFF")
        .define("GGML_NATIVE", "ON")
        .define("GGML_CCACHE", "OFF")
        .define("GGML_CUDA", "OFF")
        .define("GGML_VULKAN", "OFF")
        .define("GGML_METAL", "OFF")
        .define("GGML_SYCL", "OFF")
        .define("GGML_OPENCL", "OFF")
        .define("LLAMA_BUILD_TESTS", "OFF")
        .define("LLAMA_BUILD_TOOLS", "OFF")
        .define("LLAMA_BUILD_EXAMPLES", "OFF")
        .define("LLAMA_BUILD_SERVER", "OFF")
        .define("LLAMA_BUILD_APP", "OFF")
        .define("LLAMA_BUILD_COMMON", "OFF")
        .define("LLAMA_BUILD_IS_DEV", "OFF")
        .define("LLAMA_OPENSSL", "OFF")
        .define("LLAMA_CURL", "OFF")
        .build();

    println!(
        "cargo:rustc-link-search=native={}",
        install_dir.join("lib").display()
    );
    // Статические библиотеки llama.cpp (порядок: llama зависит от ggml*).
    for lib in ["llama", "ggml", "ggml-base", "ggml-cpu"] {
        println!("cargo:rustc-link-lib=static={lib}");
    }
    // C++-рантайм для статических C++-библиотек.
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        println!("cargo:rustc-link-lib=msvcprt");
    } else {
        println!("cargo:rustc-link-lib=stdc++");
    }
    // Windows: ggml-cpu обращается к реестру (определение CPU) — advapi32.
    if std::env::var("CARGO_CFG_WINDOWS").is_ok() {
        println!("cargo:rustc-link-lib=advapi32");
    }
}

/// Сгенерировать биндинги bindgen из `include/llama.h`.
#[cfg(feature = "native")]
fn generate_bindings(source: &Path) {
    let include = source.join("include");
    let ggml_include = source.join("ggml").join("include");
    let header = include.join("llama.h");

    let bindings = bindgen::Builder::default()
        .header(header.to_string_lossy().into_owned())
        .clang_arg(format!("-I{}", include.display()))
        .clang_arg(format!("-I{}", ggml_include.display()))
        .allowlist_function("llama_.*")
        .allowlist_type("llama_.*")
        .allowlist_var("LLAMA_.*")
        .generate_comments(false)
        .layout_tests(false)
        .generate()
        .expect("bindgen: не удалось сгенерировать биндинги (нужен LIBCLANG_PATH — каталог с libclang.dll)");

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR")).join("bindings.rs");
    bindings
        .write_to_file(&out)
        .expect("bindgen: не удалось записать bindings.rs");
}
