# tensa-llama-sys

Уровень 1 (ADR-003): raw FFI-биндинги llama.cpp. Тонкий слой, все вызовы
unsafe, изоляция в семействе `tensa-llama-*`.

## Обычная сборка (CI, без нативного слоя)

```bash
cargo build -p tensa-llama-sys
```

Крейт компилируется пустым (`NATIVE_READY == false`); `tensa-llama` при этом
честно возвращает `EngineUnavailable`.

## Нативная сборка (TASK-ENV-001)

Требования: cmake, libclang, компилятор C++ (MSVC/GCC/Clang). Исходники —
закреплённый тег `v0.7.0` в `vendor/llama.cpp` (в git не попадает).

```powershell
# 1) исходники закреплённой версии
git clone --depth 1 --branch v0.7.0 https://github.com/ggml-org/llama.cpp vendor/llama.cpp

# 2) libclang (вариант без прав администратора: pip-пакет в пользовательский Python)
python -m pip install libclang   # либо LLVM-установщик
$env:LIBCLANG_PATH = "<site-packages>\clang\native"   # каталог с libclang.dll

# 3) сборка
cargo build -p tensa-llama-sys --features native
```

- Альтернативный источник: переменная `LLAMA_CPP_DIR`.
- Сборка llama.cpp кэшируется в `target/`; при смене версии — `cargo clean -p
  tensa-llama-sys`.
- Бэкенды GPU/OpenCL выключены (CPU-инференс, ADR-004); SIMD подбирается
  `GGML_NATIVE` (на MSVC/x64 — AVX2 + FMA + F16C).
