//! Configuration management

use std::path::PathBuf;
use tokio::fs;
use tracing::{info, warn, error};
use anyhow::Result;
use directories::ProjectDirs;
use crate::ipc::Settings;

/// Settings manager for persistent storage
pub struct SettingsManager;

impl SettingsManager {
    /// Get settings file path
    fn settings_path() -> PathBuf {
        if let Some(proj_dirs) = ProjectDirs::from("com", "instantdelay", "InstantDelay") {
            proj_dirs.config_dir().join("settings.json")
        } else {
            // Fallback
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("InstantDelay")
                .join("settings.json")
        }
    }

    /// Load settings from file
    pub async fn load() -> Result<Settings> {
        let path = Self::settings_path();

        if !path.exists() {
            info!("Settings file not found, using defaults");
            return Ok(Settings::default());
        }

        let content = fs::read_to_string(&path).await?;
        let settings: Settings = serde_json::from_str(&content)?;
        info!("Settings loaded from {}", path.display());
        Ok(settings)
    }

    /// Save settings to file
    pub async fn save(settings: &Settings) -> Result<()> {
        let path = Self::settings_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }

        let content = serde_json::to_string_pretty(settings)?;
        fs::write(&path, content).await?;
        info!("Settings saved to {}", path.display());
        Ok(())
    }

    /// Reset settings to defaults
    pub async fn reset() -> Result<Settings> {
        let defaults = Settings::default();
        Self::save(&defaults).await?;
        Ok(defaults)
    }

    /// Export settings to file
    pub async fn export(settings: &Settings, path: PathBuf) -> Result<()> {
        let content = serde_json::to_string_pretty(settings)?;
        fs::write(path, content).await?;
        Ok(())
    }

    /// Import settings from file
    pub async fn import(path: PathBuf) -> Result<Settings> {
        let content = fs::read_to_string(path).await?;
        let settings: Settings = serde_json::from_str(&content)?;
        Ok(settings)
    }
}