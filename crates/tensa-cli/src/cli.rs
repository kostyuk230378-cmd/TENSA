//! Разбор аргументов командной строки (clap; дерево команд — по ТЗ tensa-cli).
//!
//! Глобальные опции: `-c/--config` (путь к `config.toml`), `-v/--verbose`
//! (tracing debug), `--json`, `--quiet`.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

/// tensa — CLI управления и отладки ядра.
#[derive(Debug, Parser)]
#[command(
    name = "tensa",
    version,
    about = "Tensa CLI: управление и отладка ядра",
    long_about = None
)]
pub struct Cli {
    /// Путь к config.toml (по умолчанию ~/.tensa/config.toml)
    #[arg(short = 'c', long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Подробный вывод (уровень debug)
    #[arg(short = 'v', long, global = true)]
    pub verbose: bool,

    /// Вывод машинно-читаемым JSON
    #[arg(long, global = true)]
    pub json: bool,

    /// Тихий режим: без пояснений, только данные и код возврата
    #[arg(long, global = true)]
    pub quiet: bool,

    #[command(subcommand)]
    pub command: Command,
}

/// Язык интерфейса для мастера первого запуска.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LanguageChoice {
    /// Русский
    Ru,
    /// English
    En,
}

impl From<LanguageChoice> for tensa_core::Language {
    fn from(value: LanguageChoice) -> Self {
        match value {
            LanguageChoice::Ru => tensa_core::Language::Ru,
            LanguageChoice::En => tensa_core::Language::En,
        }
    }
}

/// Команды верхнего уровня.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Первый запуск: записать config.toml и secrets.toml
    Init(InitArgs),
    /// Запустить ядро как процесс (не реализовано в CLI MVP)
    Start,
    /// Остановить ядро (не реализовано в CLI MVP)
    Stop,
    /// Сводный статус: пути, узел, счётчики
    Status,
    /// Проверка здоровья ядра
    Health,
    /// Конфигурация: get/set/list/reload
    Config(ConfigArgs),
    /// Очередь одобрения: list/show/approve/reject
    Approval(ApprovalArgs),
    /// Память: index/reindex/delete/search/list
    Memory(MemoryArgs),
    /// Заметки: create/list/search/delete
    Notes(NotesArgs),
    /// Контакты: create/list/search/delete
    Contacts(ContactsArgs),
    /// Почта: connect/disconnect/folders/fetch/search/send
    Email(EmailArgs),
    /// Календарь: list/conflicts/create/delete
    Calendar(CalendarArgs),
    /// Приложения: list/scan/launch/open
    Apps(AppsArgs),
    /// Узлы: list/get/heavy
    Nodes(NodesArgs),
    /// Синхронизация: start/stop/status/push/pull
    Sync(SyncArgs),
    /// Запрос к моделям (текст)
    Query(QueryArgs),
    /// Распознавание речи из файла
    Transcribe(TranscribeArgs),
    /// Бенчмарк устройства (tensa-perf)
    Benchmark(BenchmarkArgs),
    /// Версия и окружение сборки
    Version,
}

/// Аргументы `init`.
#[derive(Debug, Args)]
pub struct InitArgs {
    /// Язык интерфейса
    #[arg(long, value_enum, default_value_t = LanguageChoice::Ru)]
    pub language: LanguageChoice,

    /// Имя владельца
    #[arg(long, value_name = "NAME")]
    pub owner_name: String,

    /// Как обращаться к владельцу
    #[arg(long, value_name = "TITLE", default_value = "")]
    pub owner_title: String,

    /// Имя секретаря
    #[arg(long, value_name = "NAME", default_value = "Tensa")]
    pub secretary_name: String,

    /// Часовой пояс (IANA)
    #[arg(long, value_name = "TZ", default_value = "UTC")]
    pub timezone: String,

    /// Отметить согласие на загрузку моделей (шаг 3 мастера)
    #[arg(long)]
    pub download_models: bool,
}

/// Аргументы `config`.
#[derive(Debug, Args)]
pub struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

/// Подкоманды `config`.
#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Показать значение по точечному ключу (general.language)
    Get {
        /// Точечный ключ TOML
        key: String,
    },
    /// Записать значение (bool/int/float/строка — по виду значения)
    Set {
        /// Точечный ключ TOML
        key: String,
        /// Новое значение
        value: String,
    },
    /// Напечатать конфигурацию целиком
    List,
    /// Проверить и перечитать конфигурацию с диска
    Reload,
}

/// Аргументы `approval`.
#[derive(Debug, Args)]
pub struct ApprovalArgs {
    #[command(subcommand)]
    pub command: ApprovalCommand,
}

/// Подкоманды `approval`.
#[derive(Debug, Subcommand)]
pub enum ApprovalCommand {
    /// Ожидающие решения действия
    List,
    /// Показать действие по id
    Show {
        /// UUID действия
        id: String,
    },
    /// Подтвердить действие
    Approve {
        /// UUID действия
        id: String,
        /// Кто подтвердил (user/telegram/mobile/desktop)
        #[arg(long, default_value = "user")]
        by: String,
    },
    /// Отклонить действие
    Reject {
        /// UUID действия
        id: String,
        /// Кто отклонил (user/telegram/mobile/desktop)
        #[arg(long, default_value = "user")]
        by: String,
    },
}

/// Аргументы `memory`.
#[derive(Debug, Args)]
pub struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

/// Подкоманды `memory`.
#[derive(Debug, Subcommand)]
pub enum MemoryCommand {
    /// Проиндексировать документ
    Index {
        /// Файл для индексации
        path: PathBuf,
    },
    /// Переиндексировать всё
    Reindex,
    /// Удалить запись памяти
    Delete {
        /// UUID записи
        id: String,
    },
    /// Поиск по памяти
    Search {
        /// Поисковый запрос
        query: String,
        /// Максимум результатов
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Оглавление памяти
    List,
}

/// Аргументы `notes`.
#[derive(Debug, Args)]
pub struct NotesArgs {
    #[command(subcommand)]
    pub command: NotesCommand,
}

/// Подкоманды `notes`.
#[derive(Debug, Subcommand)]
pub enum NotesCommand {
    /// Создать заметку
    Create {
        /// Заголовок
        title: String,
        /// Текст заметки
        #[arg(long, default_value = "")]
        body: String,
        /// Тег (можно несколько раз)
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
    },
    /// Список заметок
    List {
        /// Максимум заметок
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Смещение
        #[arg(long, default_value_t = 0)]
        offset: usize,
    },
    /// Поиск (FTS5)
    Search {
        /// Запрос
        query: String,
        /// Максимум результатов
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Удалить заметку (мягкое удаление)
    Delete {
        /// UUID заметки
        id: String,
    },
}

/// Аргументы `contacts`.
#[derive(Debug, Args)]
pub struct ContactsArgs {
    #[command(subcommand)]
    pub command: ContactsCommand,
}

/// Подкоманды `contacts`.
#[derive(Debug, Subcommand)]
pub enum ContactsCommand {
    /// Создать контакт
    Create {
        /// Имя
        name: String,
        /// Почта
        #[arg(long)]
        email: Option<String>,
        /// Телефон
        #[arg(long)]
        phone: Option<String>,
        /// Тег (можно несколько раз)
        #[arg(long = "tag", value_name = "TAG")]
        tags: Vec<String>,
    },
    /// Список контактов
    List {
        /// Максимум контактов
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Смещение
        #[arg(long, default_value_t = 0)]
        offset: usize,
    },
    /// Поиск по имени/полям
    Search {
        /// Запрос
        query: String,
        /// Максимум результатов
        #[arg(long, default_value_t = 10)]
        limit: usize,
    },
    /// Удалить контакт
    Delete {
        /// UUID контакта
        id: String,
    },
}

/// Аргументы `email`.
#[derive(Debug, Args)]
pub struct EmailArgs {
    #[command(subcommand)]
    pub command: EmailCommand,
}

/// Подкоманды `email` (Контракт 6; реализация — этап tensa-tools).
#[derive(Debug, Subcommand)]
pub enum EmailCommand {
    /// Подключить аккаунт
    Connect {
        /// id аккаунта из config.toml
        account: String,
    },
    /// Отключить аккаунт
    Disconnect {
        /// id аккаунта
        account: String,
    },
    /// Папки аккаунта
    Folders {
        /// id аккаунта
        account: String,
    },
    /// Загрузить письма
    Fetch {
        /// id аккаунта
        account: String,
        /// Сколько писем
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Поиск писем
    Search {
        /// id аккаунта
        account: String,
        /// Запрос
        query: String,
    },
    /// Отправить письмо (через ApprovalStore)
    Send {
        /// Получатель
        to: String,
        /// Тема
        subject: String,
        /// Текст
        body: String,
    },
}

/// Аргументы `calendar`.
#[derive(Debug, Args)]
pub struct CalendarArgs {
    #[command(subcommand)]
    pub command: CalendarCommand,
}

/// Подкоманды `calendar` (реализация — этап tensa-tools).
#[derive(Debug, Subcommand)]
pub enum CalendarCommand {
    /// События за период
    List {
        /// Начало периода (ISO 8601)
        #[arg(long)]
        from: Option<String>,
        /// Конец периода (ISO 8601)
        #[arg(long)]
        to: Option<String>,
    },
    /// Конфликты в календаре
    Conflicts,
    /// Создать событие (через ApprovalStore)
    Create {
        /// Название
        title: String,
        /// Начало (ISO 8601)
        #[arg(long)]
        start: String,
        /// Конец (ISO 8601)
        #[arg(long)]
        end: String,
    },
    /// Удалить событие
    Delete {
        /// id события
        id: String,
    },
}

/// Аргументы `apps`.
#[derive(Debug, Args)]
pub struct AppsArgs {
    #[command(subcommand)]
    pub command: AppsCommand,
}

/// Подкоманды `apps` (ТЗ tensa-tools: apps).
#[derive(Debug, Subcommand)]
pub enum AppsCommand {
    /// Установленные приложения
    List,
    /// Пересканировать приложения
    Scan,
    /// Запустить приложение
    Launch {
        /// Имя приложения
        name: String,
    },
    /// Открыть файл или URL системным обработчиком
    Open {
        /// Файл или URL
        target: String,
    },
}

/// Аргументы `nodes`.
#[derive(Debug, Args)]
pub struct NodesArgs {
    #[command(subcommand)]
    pub command: NodesCommand,
}

/// Подкоманды `nodes` (Контракт: сеть узлов).
#[derive(Debug, Subcommand)]
pub enum NodesCommand {
    /// Все узлы из реестра
    List,
    /// Показать узел по id
    Get {
        /// UUID узла
        id: String,
    },
    /// Онлайн heavy-узел для инференса
    Heavy,
}

/// Аргументы `sync`.
#[derive(Debug, Args)]
pub struct SyncArgs {
    #[command(subcommand)]
    pub command: SyncCommand,
}

/// Подкоманды `sync` (реализация — этап tensa-sync).
#[derive(Debug, Subcommand)]
pub enum SyncCommand {
    /// Запустить синхронизацию
    Start,
    /// Остановить синхронизацию
    Stop,
    /// Статус синхронизации
    Status,
    /// Отправить изменения
    Push,
    /// Принять изменения
    Pull,
}

/// Аргументы `query`.
#[derive(Debug, Args)]
pub struct QueryArgs {
    /// Текст запроса
    pub text: String,
}

/// Аргументы `transcribe`.
#[derive(Debug, Args)]
pub struct TranscribeArgs {
    /// Аудиофайл
    pub file: PathBuf,
}

/// Аргументы `benchmark`.
#[derive(Debug, Args)]
pub struct BenchmarkArgs {
    /// Каталог для дисковых проб (по умолчанию — корень данных)
    #[arg(long, value_name = "DIR")]
    pub dir: Option<PathBuf>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// clap сам проверяет корректность дерева команд (уникальность, конфликты).
    #[test]
    fn command_tree_is_valid() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_global_flags_and_config() {
        let cli = Cli::try_parse_from([
            "tensa",
            "-c",
            "/tmp/config.toml",
            "--json",
            "notes",
            "list",
            "--limit",
            "5",
        ])
        .expect("разбор");
        assert_eq!(
            cli.config.as_deref(),
            Some(std::path::Path::new("/tmp/config.toml"))
        );
        assert!(cli.json);
        match cli.command {
            Command::Notes(NotesArgs {
                command: NotesCommand::List { limit, offset },
            }) => {
                assert_eq!(limit, 5);
                assert_eq!(offset, 0);
            }
            other => panic!("не та команда: {other:?}"),
        }
    }

    #[test]
    fn parses_init_arguments() {
        let cli = Cli::try_parse_from([
            "tensa",
            "init",
            "--owner-name",
            "Иван",
            "--secretary-name",
            "Tensa",
            "--timezone",
            "Europe/Moscow",
            "--language",
            "ru",
        ])
        .expect("разбор");
        match cli.command {
            Command::Init(args) => {
                assert_eq!(args.owner_name, "Иван");
                assert_eq!(args.secretary_name, "Tensa");
                assert_eq!(args.language, LanguageChoice::Ru);
                assert!(!args.download_models);
            }
            other => panic!("не та команда: {other:?}"),
        }
    }

    #[test]
    fn missing_required_argument_fails() {
        // init без --owner-name — ошибка разбора (exit code 2 у clap).
        assert!(Cli::try_parse_from(["tensa", "init"]).is_err());
    }
}
