use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

/// Pool preset options
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PoolPreset {
    Ckpool,
    Braiins,
    Ocean,
    None,
    Custom,
}

impl Default for PoolPreset {
    fn default() -> Self {
        Self::Ckpool
    }
}

impl PoolPreset {
    /// Get the pool address for this preset
    pub fn address(&self) -> Option<&'static str> {
        match self {
            Self::Ckpool => Some("solo.ckpool.org:3333"),
            Self::Braiins => Some("solo.stratum.braiins.com:3333"),
            Self::Ocean => Some("mine.ocean.xyz:3334"),
            Self::None => None,
            Self::Custom => None,
        }
    }

    /// Get the stratum version for this preset
    pub fn stratum_version(&self) -> StratumVersion {
        match self {
            Self::Ckpool | Self::Braiins | Self::Ocean => StratumVersion::V1,
            Self::None | Self::Custom => StratumVersion::V1,
        }
    }

    /// Check if this preset requires Bitcoin Core
    pub fn requires_bitcoind(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Stratum protocol version
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum StratumVersion {
    #[default]
    V1,
    V2,
}

/// Pool configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PoolConfig {
    #[serde(default)]
    pub preset: PoolPreset,
    pub address: Option<String>,
    pub payout_address: String,
    #[serde(default = "default_password")]
    pub password: String,
    pub stratum_version: Option<StratumVersion>,
}

fn default_password() -> String {
    "x".to_string()
}

impl PoolConfig {
    /// Get the effective pool address
    pub fn effective_address(&self) -> Option<String> {
        if self.preset == PoolPreset::Custom {
            self.address.clone()
        } else {
            self.preset.address().map(String::from)
        }
    }

    /// Get the effective stratum version
    pub fn effective_stratum_version(&self) -> StratumVersion {
        self.stratum_version.unwrap_or_else(|| self.preset.stratum_version())
    }
}

/// Mining configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MiningConfig {
    #[serde(default = "default_true")]
    pub enable_cpu: bool,
    #[serde(default)]
    pub threads: usize,
    #[serde(default)]
    pub enable_gpu: bool,
    #[serde(default)]
    pub gpu_devices: Vec<u32>,
    #[serde(default = "default_gpu_intensity")]
    pub gpu_intensity: u8,
}

fn default_true() -> bool {
    true
}

fn default_gpu_intensity() -> u8 {
    80
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            enable_cpu: true,
            threads: 0,
            enable_gpu: false,
            gpu_devices: vec![],
            gpu_intensity: 80,
        }
    }
}

/// Bitcoin Core configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BitcoindConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub managed: bool,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_rpc_port")]
    pub rpc_port: u16,
    pub rpc_user: Option<String>,
    pub rpc_password: Option<String>,
    pub datadir: Option<String>,
    #[serde(default = "default_prune")]
    pub prune: u32,
}

fn default_host() -> String {
    "127.0.0.1".to_string()
}

fn default_rpc_port() -> u16 {
    8332
}

fn default_prune() -> u32 {
    10000
}

impl Default for BitcoindConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            managed: false,
            host: default_host(),
            rpc_port: default_rpc_port(),
            rpc_user: None,
            rpc_password: None,
            datadir: None,
            prune: default_prune(),
        }
    }
}

/// Stratum V2 configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Sv2Config {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_host")]
    pub proxy_host: String,
    #[serde(default = "default_sv2_port")]
    pub proxy_port: u16,
}

fn default_sv2_port() -> u16 {
    34255
}

impl Default for Sv2Config {
    fn default() -> Self {
        Self {
            enabled: false,
            proxy_host: default_host(),
            proxy_port: default_sv2_port(),
        }
    }
}

/// Web interface configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WebConfig {
    #[serde(default = "default_web_port")]
    pub port: u16,
    #[serde(default = "default_web_dir")]
    pub dir: String,
}

fn default_web_port() -> u16 {
    8080
}

fn default_web_dir() -> String {
    "web".to_string()
}

impl Default for WebConfig {
    fn default() -> Self {
        Self {
            port: default_web_port(),
            dir: default_web_dir(),
        }
    }
}

/// Log output format
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogFormat {
    #[default]
    Text,
    Json,
}

/// Logging configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LoggingConfig {
    #[serde(default = "default_stats_interval")]
    pub stats_interval: u32,
    #[serde(default = "default_true")]
    pub colors: bool,
    #[serde(default)]
    pub file: Option<String>,
    #[serde(default = "default_log_level")]
    pub level: String,
    #[serde(default = "default_true")]
    pub system_bell: bool,
    /// Enable TUI dashboard (disable with no_hud = true)
    #[serde(default = "default_true")]
    pub hud: bool,
    /// Log output format (text or json)
    #[serde(default)]
    pub format: LogFormat,
}

fn default_stats_interval() -> u32 {
    10
}

fn default_log_level() -> String {
    "info".to_string()
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            stats_interval: default_stats_interval(),
            colors: true,
            file: None,
            level: default_log_level(),
            system_bell: true,
            hud: true,
            format: LogFormat::Text,
        }
    }
}

/// Network configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct NetworkConfig {
    #[serde(default = "default_reconnect_delay")]
    pub reconnect_delay: u32,
    #[serde(default = "default_max_reconnect_delay")]
    pub max_reconnect_delay: u32,
}

fn default_reconnect_delay() -> u32 {
    5
}

fn default_max_reconnect_delay() -> u32 {
    300
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            reconnect_delay: default_reconnect_delay(),
            max_reconnect_delay: default_max_reconnect_delay(),
        }
    }
}

/// Idle/screensaver mode configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct IdleConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_idle_timeout")]
    pub timeout: u32,
    #[serde(default = "default_true")]
    pub pause_on_active: bool,
}

fn default_idle_timeout() -> u32 {
    900 // 15 minutes
}

impl Default for IdleConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            timeout: default_idle_timeout(),
            pause_on_active: true,
        }
    }
}

/// Main configuration structure
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub pool: PoolConfig,
    #[serde(default)]
    pub mining: MiningConfig,
    #[serde(default)]
    pub bitcoind: BitcoindConfig,
    #[serde(default)]
    pub sv2: Sv2Config,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub network: NetworkConfig,
    #[serde(default)]
    pub idle: IdleConfig,
}

impl Config {
    /// Load configuration from a TOML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), ConfigError> {
        // Check payout address
        if self.pool.payout_address.is_empty()
            || self.pool.payout_address == "YOUR_BTC_ADDRESS_HERE"
        {
            return Err(ConfigError::ValidationError(
                "payout_address must be set to a valid Bitcoin address".to_string(),
            ));
        }

        // Check if bitcoind is enabled when required
        if self.pool.preset.requires_bitcoind() && !self.bitcoind.enabled {
            return Err(ConfigError::ValidationError(
                "Bitcoin Core must be enabled (bitcoind.enabled = true) for solo mode".to_string(),
            ));
        }

        // Check custom pool address
        if self.pool.preset == PoolPreset::Custom && self.pool.address.is_none() {
            return Err(ConfigError::ValidationError(
                "pool.address must be set when using custom preset".to_string(),
            ));
        }

        // Check mining settings
        if !self.mining.enable_cpu && !self.mining.enable_gpu {
            return Err(ConfigError::ValidationError(
                "At least one of enable_cpu or enable_gpu must be true".to_string(),
            ));
        }

        Ok(())
    }

    /// Check if running in solo mode (no pool)
    pub fn is_solo_mode(&self) -> bool {
        self.pool.preset == PoolPreset::None
    }

    /// Check if SV2 is active
    pub fn is_sv2_mode(&self) -> bool {
        self.sv2.enabled
            || self.pool.effective_stratum_version() == StratumVersion::V2
    }

    /// Get effective thread count
    pub fn effective_threads(&self) -> usize {
        if self.mining.threads == 0 {
            num_cpus::get()
        } else {
            self.mining.threads
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_preset_addresses() {
        assert_eq!(PoolPreset::Ckpool.address(), Some("solo.ckpool.org:3333"));
        assert_eq!(PoolPreset::Braiins.address(), Some("solo.stratum.braiins.com:3333"));
        assert_eq!(PoolPreset::Ocean.address(), Some("mine.ocean.xyz:3334"));
        assert_eq!(PoolPreset::None.address(), None);
    }

    #[test]
    fn test_config_validation_missing_address() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = ""
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_solo_without_bitcoind() {
        let toml_content = r#"
            [pool]
            preset = "none"
            payout_address = "bc1qtest"

            [bitcoind]
            enabled = false
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_valid() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_log_format_default() {
        let logging = LoggingConfig::default();
        assert_eq!(logging.format, LogFormat::Text);
        assert!(logging.hud);
    }

    #[test]
    fn test_log_format_json() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [logging]
            format = "json"
            hud = false
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.logging.format, LogFormat::Json);
        assert!(!config.logging.hud);
    }

    #[test]
    fn test_log_format_text_explicit() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [logging]
            format = "text"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.logging.format, LogFormat::Text);
    }

    #[test]
    fn test_idle_config_default() {
        let idle = IdleConfig::default();
        assert!(!idle.enabled);
        assert_eq!(idle.timeout, 900);
        assert!(idle.pause_on_active);
    }

    #[test]
    fn test_idle_config_custom() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [idle]
            enabled = true
            timeout = 300
            pause_on_active = false
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.idle.enabled);
        assert_eq!(config.idle.timeout, 300);
        assert!(!config.idle.pause_on_active);
    }
}
