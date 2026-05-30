use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub nickname: NicknameConfig,
    pub network: NetworkConfig,
    pub relay: RelayConfig,
    pub bandwidth: BandwidthConfig,
    pub advanced: AdvancedConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NicknameConfig {
    pub value: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub room: String,
    pub password: String,
    pub autostart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayConfig {
    pub enable: bool,
    pub max_relay_connections: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BandwidthConfig {
    pub broadcast_limit: u32,
    pub max_peers: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdvancedConfig {
    pub virtual_ip_range: String,
    pub use_dht: bool,
    pub hole_punch_timeout_sec: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            nickname: NicknameConfig {
                value: format!("Player{}", rand::random::<u16>()),
            },
            network: NetworkConfig {
                room: "public".into(),
                password: String::new(),
                autostart: false,
            },
            relay: RelayConfig {
                enable: true,
                max_relay_connections: 8,
            },
            bandwidth: BandwidthConfig {
                broadcast_limit: 10,
                max_peers: 64,
            },
            advanced: AdvancedConfig {
                virtual_ip_range: "10.144.0.0/16".into(),
                use_dht: true,
                hole_punch_timeout_sec: 5,
            },
        }
    }
}

impl Config {
    pub fn config_dir() -> anyhow::Result<PathBuf> {
        let dir = directories::ProjectDirs::from("com", "vlkxn", "Vlkxn")
            .ok_or_else(|| anyhow::anyhow!("Cannot determine config directory"))?;
        Ok(dir.config_dir().to_path_buf())
    }

    pub fn config_path() -> anyhow::Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn keys_path() -> anyhow::Result<PathBuf> {
        Ok(Self::config_dir()?.join("keys.json"))
    }

    pub fn load() -> anyhow::Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            Ok(toml::from_str(&content)?)
        } else {
            let config = Config::default();
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let content = toml::to_string_pretty(&config)?;
            std::fs::write(&path, content)?;
            Ok(config)
        }
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
