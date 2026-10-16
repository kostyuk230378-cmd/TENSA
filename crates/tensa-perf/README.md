# tensa-perf

Профилирование устройства и динамическое распределение ролей heavy/thin
(ТЗ tensa-perf; ADR-004, ADR-012, ADR-013).

## Назначение

- снимок устройства (`DeviceProfile`): RAM, CPU (ядра/частота/загрузка),
  GPU, NPU, диски, батарея, thermal, ОС/архитектура;
- бенчмарк (`BenchmarkResult`): пропускная способность памяти, чтение/запись
  диска, CPU-скор; кэш `~/.tensa/benchmark.json`;
- решение о роли (`NodeRoleDecision`): heavy/thin по порогам `PerfConfig`
  (RAM ≥ 8 ГБ, ≥ 4 ядер, батарея > 20 %, thermal ≤ Serious), рекомендованный
  тир модели (XS/S/M/L), доступность LLM/памяти/Telegram/почты;
- фоновый мониторинг (интервал из конфига, по умолчанию 60 с).

## Статус источников

- CPU/RAM/диски — `sysinfo` (полностью);
- батарея — Win32 `GetSystemPowerStatus` / Linux+Android sysfs (полностью);
- thermal — Linux+Android sysfs; на Windows сенсоров нет — `Nominal`
  (см. `thermal::supported()`);
- GPU (NVML) и NPU (NNAPI/CoreML/DirectML) — подключаются вместе с нативным
  слоем на TASK-ENV-001; до этого детекция возвращает `None` без выдуманных
  данных (`gpu::detection_available() == false`).

## Тесты

```bash
cargo test -p tensa-perf
```

## Ограничения

Не маршрутизирует, не синхронизирует, не логирует секреты (ТЗ).
