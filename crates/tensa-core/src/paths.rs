//! Пути Tensa (`~/.tensa/...`). Контракт 4; ТЗ tensa-core, модуль paths.rs.

use std::path::PathBuf;

/// Базовые пути. Явный override — для тестов и CLI (`--config`).
#[derive(Debug, Clone)]
pub struct TensaPaths {
    root: PathBuf,
}

impl TensaPaths {
    /// Пути по умолчанию: `~/.tensa/` — домашний каталог пользователя
    /// (Контракт 4: `~/.tensa/approval.db`, `~/.tensa/config.toml`,
    /// `~/.tensa/memory/`, `~/.tensa/models/` и т. д.).
    ///
    /// Платформенные каталоги (`directories`) не используются: контракт
    /// фиксирует именно `~/.tensa/`. Фолбэк при отсутствии переменных
    /// окружения — `./.tensa`.
    pub fn default_root() -> PathBuf {
        let home = std::env::var("USERPROFILE")
            .or_else(|_| std::env::var("HOME"))
            .unwrap_or_else(|_| ".".to_string());
        PathBuf::from(home).join(".tensa")
    }

    pub fn new() -> Self {
        Self {
            root: Self::default_root(),
        }
    }

    /// Пути с явным корнем (тесты, портативный режим).
    pub fn with_root(root: PathBuf) -> Self {
        Self { root }
    }

    pub fn root(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn secrets_file(&self) -> PathBuf {
        self.root.join("secrets.toml")
    }

    pub fn approval_db(&self) -> PathBuf {
        self.root.join("approval.db")
    }

    pub fn notes_db(&self) -> PathBuf {
        self.root.join("notes.db")
    }

    pub fn memory_dir(&self) -> PathBuf {
        self.root.join("memory")
    }

    pub fn models_dir(&self) -> PathBuf {
        self.root.join("models")
    }

    /// Создать структуру каталогов (без секретов).
    pub fn ensure_dirs(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(&self.root)?;
        std::fs::create_dir_all(self.memory_dir())?;
        std::fs::create_dir_all(self.models_dir())?;
        Ok(())
    }
}

impl Default for TensaPaths {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_composition() {
        let p = TensaPaths::with_root(PathBuf::from("/tmp/tensa-test"));
        assert_eq!(
            p.config_file(),
            PathBuf::from("/tmp/tensa-test/config.toml")
        );
        assert_eq!(
            p.secrets_file(),
            PathBuf::from("/tmp/tensa-test/secrets.toml")
        );
        assert_eq!(p.notes_db(), PathBuf::from("/tmp/tensa-test/notes.db"));
        assert!(p.memory_dir().ends_with("memory"));
    }

    #[test]
    fn default_root_is_dot_tensa_in_home() {
        let root = TensaPaths::default_root();
        assert_eq!(
            root.file_name().and_then(|name| name.to_str()),
            Some(".tensa")
        );
        if let Ok(home) = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")) {
            assert_eq!(root, PathBuf::from(home).join(".tensa"));
        }
    }

    #[test]
    fn ensure_dirs_creates() {
        let root = std::env::temp_dir().join(format!("tensa-paths-{}", uuid::Uuid::new_v4()));
        let p = TensaPaths::with_root(root.clone());
        p.ensure_dirs().expect("dirs");
        assert!(root.join("memory").is_dir());
        assert!(root.join("models").is_dir());
        std::fs::remove_dir_all(root).ok();
    }
}
