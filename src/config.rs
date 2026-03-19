use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub general: GeneralConfig,
    pub mouse: MouseConfig,
    pub theme: ThemeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub docker_host: String,
    pub tick_rate_ms: u64,
    pub default_tab: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct MouseConfig {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    pub name: String,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            docker_host: "unix:///var/run/docker.sock".to_string(),
            tick_rate_ms: 250,
            default_tab: "containers".to_string(),
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
        }
    }
}

impl AppConfig {
    /// Load config from the given path, or the default path, falling back to defaults.
    pub fn load(config_path: Option<&str>) -> Result<Self> {
        let path = if let Some(p) = config_path {
            PathBuf::from(p)
        } else {
            Self::default_config_path()
        };

        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: AppConfig = toml::from_str(&content)?;
            Ok(config)
        } else {
            Ok(AppConfig::default())
        }
    }

    fn default_config_path() -> PathBuf {
        dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("docsee")
            .join("config.toml")
    }
}
