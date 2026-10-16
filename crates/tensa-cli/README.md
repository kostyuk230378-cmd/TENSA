# tensa-cli

CLI для отладки и управления Tensa (ТЗ tensa-cli): бинарь `tensa`.

Принципы: логика — в крейтах, CLI только вызывает; состояние между запусками
не хранится (каждый вызов — процесс, доступ к ядру in-process); секреты
не логируются.

## Использование

```powershell
tensa -c ~/.tensa/config.toml status
tensa --json health
tensa init --owner-name Иван --secretary-name Tensa --timezone Europe/Moscow
tensa notes create "Список дел" --body "Позвонить в банк" --tag дом
tensa approval list
tensa benchmark --json
```

Глобальные опции: `-c/--config <PATH>`, `-v/--verbose`, `--json`, `--quiet`.

## Состояние команд

| Команда | Статус |
| --- | --- |
| `init` | Реализовано: мастер первого запуска (config.toml + secrets.toml) |
| `status`, `health` | Реализовано: сводка и `HealthStatus` ядра |
| `config get/set/list/reload` | Реализовано (set: валидация + бэкап `*.toml.bak`) |
| `approval list/show/approve/reject` | Реализовано (Контракт 1) |
| `notes create/list/search/delete` | Реализовано (Контракт 3, FTS5) |
| `contacts create/list/search/delete` | Реализовано (Контракт 3) |
| `nodes list/get/heavy` | Реализовано (реестр узлов) |
| `benchmark` | Реализовано (tensa-perf) |
| `version` | Реализовано |
| `memory …` | Заглушка: проводка tensa-memory в CLI — в очереди |
| `email/calendar/apps/sync …` | Заглушка: этапы tensa-tools / tensa-sync |
| `query`, `transcribe` | Заглушка: нативная сборка LLM / голос — в очереди |
| `start`, `stop` | Заглушка: демон — tensa-server/apps (CLI работает in-process) |

Заглушки отвечают кодом возврата 1 и честным сообщением «не реализовано в
tensa-cli (MVP): …» — клиенты отличают «ещё нет» от прочих ошибок.

## Проверки

```powershell
cargo test -p tensa-cli        # unit (clap-дерево, вывод) + интеграционные (бинарь)
cargo clippy -p tensa-cli --all-targets -- -D warnings
```

Интеграционные тесты запускают реальный бинарь в временных корнях
(`-c <tmp>/config.toml`); домашний `~/.tensa` не затрагивается.
