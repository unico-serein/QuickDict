use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use anyhow::Result;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub dictionary_path: PathBuf,
    pub mdx_file: Option<PathBuf>,
    pub mdd_file: Option<PathBuf>,
    pub css_file: Option<PathBuf>,
    pub hotkey: String,
    pub clipboard_monitor: bool,
    pub display: DisplaySettings,
    pub window: WindowSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplaySettings {
    pub font_family: String,
    pub font_size: String,
    pub line_height: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSettings {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            dictionary_path: PathBuf::from(""),
            mdx_file: None,
            mdd_file: None,
            css_file: None,
            hotkey: "Alt+M".to_string(),
            clipboard_monitor: false,
            display: DisplaySettings::default(),
            window: WindowSettings::default(),
        }
    }
}

impl Default for DisplaySettings {
    fn default() -> Self {
        Self {
            font_family: "Segoe UI".to_string(),
            font_size: "14".to_string(),
            line_height: "1.6".to_string(),
        }
    }
}

impl Default for WindowSettings {
    fn default() -> Self {
        Self {
            width: 600,
            height: 52,
            x: None,
            y: None,
        }
    }
}

impl AppConfig {
    pub fn config_dir() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("rdict")
    }

    pub fn config_file() -> PathBuf {
        Self::config_dir().join("config.json")
    }

    pub fn load() -> Result<Self> {
        let config_path = Self::config_file();
        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path)?;
            let config: AppConfig = serde_json::from_str(&content)?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir();
        std::fs::create_dir_all(&config_dir)?;
        let config_path = Self::config_file();
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(&config_path, content)?;
        Ok(())
    }

    pub fn update_dictionary_path(&mut self, path: PathBuf) {
        self.dictionary_path = path.clone();
        // Auto-detect dictionary files
        if let Ok(entries) = std::fs::read_dir(&path) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    match ext.to_str() {
                        Some("mdx") => self.mdx_file = Some(path),
                        Some("mdd") => self.mdd_file = Some(path),
                        Some("css") => self.css_file = Some(path),
                        _ => {}
                    }
                }
            }
        }
    }
}
