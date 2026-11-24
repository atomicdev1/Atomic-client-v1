use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::engine::paths::Dirs;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Settings {
    /// Azure application (public client) id used for Microsoft device-code login.
    pub client_id: String,
    /// JVM max heap in MB.
    pub memory_mb: u32,
    /// Fallback offline username when no Microsoft account is signed in.
    pub username: String,
    /// Optional explicit java executable path.
    pub java_path: String,
    /// Last selected version.
    pub last_version: String,
    /// Last selected loader: vanilla | fabric | quilt.
    pub last_loader: String,
    /// Instance name used for game directory.
    pub instance: String,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            client_id: String::new(),
            memory_mb: 4096,
            username: "Player".to_string(),
            java_path: String::new(),
            last_version: String::new(),
            last_loader: "vanilla".to_string(),
            instance: "default".to_string(),
        }
    }
}

impl Settings {
    pub fn path(dirs: &Dirs) -> std::path::PathBuf {
        dirs.root.join("settings.json")
    }

    pub fn load(dirs: &Dirs) -> Settings {
        let p = Settings::path(dirs);
        match std::fs::read_to_string(&p) {
            Ok(s) => serde_json::from_str(&s).unwrap_or_default(),
            Err(_) => Settings::default(),
        }
    }

    pub fn save(&self, dirs: &Dirs) -> Result<()> {
        let p = Settings::path(dirs);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&p, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    pub fn load_from(path: &Path) -> Settings {
        std::fs::read_to_string(path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_settings_default() {
        let s = Settings::default();
        assert_eq!(s.memory_mb, 4096);
        assert_eq!(s.username, "Player");
        assert_eq!(s.loader, "vanilla");
        assert_eq!(s.instance, "default");
        assert!(s.client_id.is_empty());
        assert!(s.java_path.is_empty());
    }

    #[test]
    fn test_settings_save_and_load() {
        let dirs = Dirs::discover();
        let mut s = Settings::default();
        s.client_id = "test_client_id".to_string();
        s.memory_mb = 2048;
        s.username = "TestUser".to_string();
        s.instance = "test_instance".to_string();

        s.save(&dirs).expect("save should succeed");

        let loaded = Settings::load(&dirs);
        assert_eq!(loaded.client_id, "test_client_id");
        assert_eq!(loaded.memory_mb, 2048);
        assert_eq!(loaded.username, "TestUser");
        assert_eq!(loaded.instance, "test_instance");
    }

    #[test]
    fn test_settings_malformed_json_fallback() {
        let dirs = Dirs::discover();
        let path = Settings::path(&dirs);
        // Write invalid JSON
        std::fs::write(&path, "{not valid json").expect("write should succeed");
        let s = Settings::load(&dirs);
        // Should fall back to defaults
        assert_eq!(s.memory_mb, 4096);
        assert_eq!(s.username, "Player");
        assert!(s.client_id.is_empty());
    }

    #[test]
    fn test_settings_missing_file_fallback() {
        let dirs = Dirs::discover();
        let path = Settings::path(&dirs);
        // Ensure file doesn't exist
        if path.exists() {
            std::fs::remove_file(&path).expect("remove should succeed");
        }
        let s = Settings::load(&dirs);
        assert_eq!(s.memory_mb, 4096);
        assert_eq!(s.username, "Player");
    }

    #[test]
    fn test_memory_mb_bounds() {
        let s = Settings::default();
        assert!(s.memory_mb >= 512);
        assert!(s.memory_mb <= 4096);
    }

    #[test]
    fn test_settings_path_creation() {
        let dirs = Dirs::discover();
        let path = Settings::path(&dirs);
        let s = Settings::default();
        s.save(&dirs).expect("save should create parent dirs and file");
        assert!(path.exists());
    }
}
