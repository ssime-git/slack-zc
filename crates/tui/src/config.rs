use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub slack: SlackConfig,
    pub zeroclaw: ZeroClawConfig,
    pub llm: LlmConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_port: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ZeroClawConfig {
    pub binary_path: String,
    pub gateway_port: u16,
    pub auto_start: bool,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub provider: String,
    pub api_key: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            slack: SlackConfig {
                client_id: String::new(),
                client_secret: String::new(),
                redirect_port: 3000,
            },
            zeroclaw: ZeroClawConfig {
                binary_path: "zeroclaw".to_string(),
                gateway_port: 8080,
                auto_start: true,
                timeout_seconds: 30,
            },
            llm: LlmConfig {
                provider: "openrouter".to_string(),
                api_key: String::new(),
            },
        }
    }
}

impl Config {
    pub fn load(path: &PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn load_or_default(path: &PathBuf) -> Self {
        Self::load(path).unwrap_or_default()
    }
}
