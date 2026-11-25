use directories::ProjectDirs;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    /// Visual scale multiplier for the cat window (0.5 .. 2.5).
    pub scale: f32,
    /// Keep the cat floating above all other windows.
    pub always_on_top: bool,
    /// Show floating white keycap badges when keys are pressed.
    pub show_keycaps: bool,
    /// Let mouse clicks pass through the cat window.
    pub click_through: bool,
    /// Saved window outer position [x, y].
    pub window_pos: Option<[f32; 2]>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            scale: 1.0,
            always_on_top: true,
            show_keycaps: true,
            click_through: false,
            window_pos: None,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> Option<PathBuf> {
        ProjectDirs::from("com", "gatin", "gatin")
            .map(|dirs| dirs.config_dir().join("config.toml"))
    }

    pub fn load() -> Self {
        let Some(path) = Self::config_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(contents) => toml::from_str(&contents).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) {
        let Some(path) = Self::config_path() else {
            return;
        };
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(serialized) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, serialized);
        }
    }
}
