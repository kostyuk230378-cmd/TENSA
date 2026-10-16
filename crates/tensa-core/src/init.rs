//! Инициализация core memory (Контракт 13, дополнение к ТЗ tensa-core).
//!
//! Хранилище core memory живёт в `tensa-memory` (координация, не
//! дублирование); здесь — параметры инициализации, тексты блоков по
//! умолчанию и обратный трейт [`MemoryInitializer`].

use crate::error::Result;
use crate::models::Language;
use serde::{Deserialize, Serialize};

/// Типы блоков core memory (Контракт 13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BlockType {
    Persona,
    Human,
    Tasks,
    Preferences,
}

impl BlockType {
    pub fn as_str(&self) -> &'static str {
        match self {
            BlockType::Persona => "persona",
            BlockType::Human => "human",
            BlockType::Tasks => "tasks",
            BlockType::Preferences => "preferences",
        }
    }
}

/// Параметры инициализации памяти (ТЗ: `init_memory(params: InitParams)`;
/// доп. ТЗ tensa-core: имя секретаря — настраиваемое).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InitParams {
    /// Имя владельца.
    pub owner_name: String,
    /// Обращение к владельцу.
    pub owner_title: String,
    /// Как называть секретаря (persona).
    pub secretary_name: String,
    /// Язык.
    pub language: Language,
    /// Часовой пояс.
    pub timezone: String,
}

impl Default for InitParams {
    fn default() -> Self {
        // Значения по умолчанию из Контракта 13 / доп. ТЗ tensa-core.
        Self {
            owner_name: "Владелец".to_string(),
            owner_title: "Шеф".to_string(),
            secretary_name: "Tensa".to_string(),
            language: Language::Ru,
            timezone: "Europe/Moscow".to_string(),
        }
    }
}

/// Тексты блоков по умолчанию (доп. ТЗ tensa-core: persona/human/tasks/
/// preferences; persona включает имя секретаря).
pub fn default_blocks(params: &InitParams) -> Vec<(BlockType, String)> {
    let language = match params.language {
        Language::Ru => "русский",
        Language::En => "english",
    };
    vec![
        (
            BlockType::Persona,
            format!(
                "Я — {secretary}, персональный ИИ-секретарь. Моя задача — помогать\n\
                 владельцу управлять документами, почтой, календарём, Telegram и\n\
                 приложениями. Я работаю на русском и английском языках. Я не\n\
                 выполняю действия без одобрения владельца. Я уважаю приватность и\n\
                 не передаю данные третьим лицам.",
                secretary = params.secretary_name
            ),
        ),
        (
            BlockType::Human,
            format!(
                "Владельца зовут {name}. Он предпочитает обращение\n\
                 «{title}». Его язык: {lang}. Часовой пояс: {tz}.",
                name = params.owner_name,
                title = params.owner_title,
                lang = language,
                tz = params.timezone,
            ),
        ),
        (BlockType::Tasks, "Активные задачи: Нет".to_string()),
        (
            BlockType::Preferences,
            "Владелец предпочитает: Нет".to_string(),
        ),
    ]
}

/// Валидация параметров (ошибки — до записи в память).
pub fn validate_params(params: &InitParams) -> Result<()> {
    if params.owner_name.trim().is_empty() {
        return Err(crate::error::CoreError::Config(
            "owner_name is empty".into(),
        ));
    }
    if params.owner_title.trim().is_empty() {
        return Err(crate::error::CoreError::Config(
            "owner_title is empty".into(),
        ));
    }
    if params.secretary_name.trim().is_empty() {
        return Err(crate::error::CoreError::Config(
            "secretary_name is empty".into(),
        ));
    }
    if params.timezone.trim().is_empty() {
        return Err(crate::error::CoreError::Config("timezone is empty".into()));
    }
    Ok(())
}

/// Обратный трейт инициализации памяти.
///
/// [derived] `tensa-memory` зависит от `tensa-core`, поэтому прямая
/// зависимость на `MemoryManager` здесь дала бы цикл; связь — через этот
/// трейт. Реализует `tensa_memory::DefaultMemoryManager`.
#[async_trait::async_trait]
pub trait MemoryInitializer: Send + Sync {
    /// Инициализировать дефолтные блоки core memory при первом запуске.
    async fn init_default(&self, params: &InitParams) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_match_contract_13() {
        let p = InitParams::default();
        assert_eq!(p.owner_name, "Владелец");
        assert_eq!(p.owner_title, "Шеф");
        assert_eq!(p.secretary_name, "Tensa");
        assert_eq!(p.timezone, "Europe/Moscow");
    }

    #[test]
    fn default_blocks_contain_all_four() {
        let blocks = default_blocks(&InitParams::default());
        assert_eq!(blocks.len(), 4);
        assert_eq!(blocks[0].0, BlockType::Persona);
        assert_eq!(blocks[1].0, BlockType::Human);
        assert_eq!(blocks[2].0, BlockType::Tasks);
        assert_eq!(blocks[3].0, BlockType::Preferences);
    }

    #[test]
    fn persona_uses_secretary_name() {
        let params = InitParams {
            secretary_name: "Ассистент".into(),
            ..InitParams::default()
        };
        let blocks = default_blocks(&params);
        assert!(blocks[0].1.contains("Ассистент"));
    }

    #[test]
    fn human_block_interpolates_params() {
        let p = InitParams {
            owner_name: "Иван".into(),
            owner_title: "Шеф".into(),
            secretary_name: "Tensa".into(),
            language: Language::Ru,
            timezone: "Europe/Minsk".into(),
        };
        let blocks = default_blocks(&p);
        let human = &blocks[1].1;
        assert!(human.contains("Иван"));
        assert!(human.contains("Шеф"));
        assert!(human.contains("Europe/Minsk"));
    }

    #[test]
    fn validation_rejects_empty_fields() {
        let p = InitParams {
            owner_name: "  ".into(),
            ..InitParams::default()
        };
        assert!(validate_params(&p).is_err());

        let p = InitParams {
            secretary_name: String::new(),
            ..InitParams::default()
        };
        assert!(validate_params(&p).is_err());

        assert!(validate_params(&InitParams::default()).is_ok());
    }
}
