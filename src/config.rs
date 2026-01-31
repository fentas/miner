use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
#[allow(clippy::enum_variant_names)] // Error suffix is idiomatic for error enums
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),
    #[error("Failed to parse config: {0}")]
    ParseError(#[from] toml::de::Error),
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

/// Pool preset options
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PoolPreset {
    #[default]
    Ckpool,
    Braiins,
    Ocean,
    None,
    Custom,
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

/// SIMD level configuration
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SimdLevelConfig {
    /// Auto-detect best available (SHA-NI > AVX2 > Generic)
    #[default]
    Auto,
    /// Intel SHA Extensions (fastest on supported CPUs)
    ShaNi,
    /// AVX2 8-way parallel (fallback for older CPUs)
    Avx2,
    /// AVX512 16-way parallel (fallback for AVX512 CPUs without SHA-NI)
    Avx512,
    /// Portable generic implementation
    Generic,
}

/// Hasher implementation type (for debugging/comparison)
/// All SHA-NI implementations perform within ~3% of each other (~272-279 MH/s @ 24 threads)
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum HasherConfig {
    /// SHA2 crate - default, recommended (272 MH/s, well-tested)
    #[default]
    Sha2,
    /// Custom SIMD intrinsics with #[target_feature] (279 MH/s, fastest)
    Simd,
    /// Manual inline assembly (203 MH/s, slowest)
    Asm,
    /// sha2-style macros with cpufeatures (273 MH/s)
    AsmV2,
    /// Global target-feature, no #[target_feature] (277 MH/s)
    /// WILL CRASH on CPUs without SHA-NI!
    AsmV3,
}

impl HasherConfig {
    /// Convert to runtime HasherType
    pub fn to_hasher_type(self) -> crate::mining::HasherType {
        match self {
            HasherConfig::Sha2 => crate::mining::HasherType::Sha2Crate,
            HasherConfig::Simd => crate::mining::HasherType::Simd,
            HasherConfig::Asm => crate::mining::HasherType::Assembly,
            HasherConfig::AsmV2 => crate::mining::HasherType::AsmV2,
            HasherConfig::AsmV3 => crate::mining::HasherType::AsmV3,
        }
    }
}

/// Mining configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MiningConfig {
    #[serde(default = "default_true")]
    pub enable_cpu: bool,
    #[serde(default)]
    pub threads: usize,
    /// CPU batch size as power of 2 (e.g., 18 = 2^18 = 256K nonces per batch)
    /// Range: 16-22, default 18. Higher = better throughput but slower job response
    #[serde(default = "default_cpu_batch_exp")]
    pub cpu_batch_exp: u8,
    /// SIMD level for CPU hashing (auto, sha-ni, avx2, avx512, generic)
    #[serde(default)]
    pub simd_level: SimdLevelConfig,
    /// Hasher implementation (simd = optimized, sha2 = baseline for debugging)
    #[serde(default)]
    pub hasher: HasherConfig,
    #[serde(default)]
    pub enable_gpu: bool,
    #[serde(default)]
    pub gpu_devices: Vec<u32>,
    #[serde(default = "default_gpu_intensity")]
    pub gpu_intensity: u8,
    /// GPU work group size (0 = auto, 64-1024 recommended)
    /// Higher values may improve performance on modern GPUs
    #[serde(default)]
    pub gpu_workgroup_size: usize,
    /// GPU batch size as power of 2 (e.g., 25 = 2^25 = 32M nonces per batch)
    /// Range: 20-28, default 25. Higher = better GPU utilization but more latency
    #[serde(default = "default_gpu_batch_exp")]
    pub gpu_batch_exp: u8,
}

fn default_true() -> bool {
    true
}

fn default_gpu_intensity() -> u8 {
    100  // Maximum intensity for best GPU performance
}

fn default_cpu_batch_exp() -> u8 {
    18  // 2^18 = 256K nonces per batch (good balance of throughput vs responsiveness)
}

fn default_gpu_batch_exp() -> u8 {
    25  // 2^25 = 32M nonces per batch
}

impl Default for MiningConfig {
    fn default() -> Self {
        Self {
            enable_cpu: true,
            threads: 0,
            cpu_batch_exp: 18,      // 256K nonces per batch
            simd_level: SimdLevelConfig::Auto,
            hasher: HasherConfig::default(),
            enable_gpu: false,
            gpu_devices: vec![],
            gpu_intensity: 100,
            gpu_workgroup_size: 0,  // Auto
            gpu_batch_exp: 25,      // 32M nonces
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
    /// Database cache size in MB (0 = auto-calculate based on available RAM)
    #[serde(default)]
    pub dbcache: u32,
    /// Enable AssumeUTXO for fast initial sync (downloads UTXO snapshot)
    #[serde(default)]
    pub assumeutxo: bool,
    /// Custom magnet URI for AssumeUTXO snapshot (optional, uses default if not set)
    #[serde(default)]
    pub assumeutxo_magnet: Option<String>,
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
            dbcache: 0, // 0 = auto-calculate
            assumeutxo: false,
            assumeutxo_magnet: None,
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
    /// Enable basic authentication (recommended when exposed to network)
    #[serde(default)]
    pub auth_enabled: bool,
    /// Username for basic auth (required if auth_enabled)
    pub auth_user: Option<String>,
    /// Password for basic auth (required if auth_enabled)
    pub auth_password: Option<String>,
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
            auth_enabled: false,
            auth_user: None,
            auth_password: None,
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
    /// Idle CPU mining (default: true = CPU pauses when user is active)
    #[serde(default = "default_true")]
    pub cpu: bool,
    /// Idle GPU mining (default: true = GPU pauses when user is active)
    #[serde(default = "default_true")]
    pub gpu: bool,
    /// Inverse GPU idle mode (default: false)
    /// When true: GPU runs when user is ACTIVE, pauses when IDLE
    /// Useful when GPU affects CPU performance - run GPU only when user is present
    #[serde(default)]
    pub gpu_inverse: bool,
    /// Specific GPU devices to idle (empty = all GPUs follow idle setting)
    #[serde(default)]
    pub gpu_devices: Vec<u32>,
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
            cpu: true,
            gpu: true,
            gpu_inverse: false,
            gpu_devices: vec![],
        }
    }
}

/// Overlay window configuration (requires --features overlay)
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct OverlayConfig {
    /// Auto-open overlay on startup
    #[serde(default)]
    pub auto_open: bool,
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
    #[serde(default)]
    pub overlay: OverlayConfig,
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

        // Check for conflicting bitcoind settings
        if self.bitcoind.enabled && self.bitcoind.managed && self.pool.preset != PoolPreset::None {
            return Err(ConfigError::ValidationError(
                "bitcoind.managed=true only works with pool.preset=\"none\" (solo mining)".to_string(),
            ));
        }

        // Check external RPC credentials when not managed
        if self.bitcoind.enabled && !self.bitcoind.managed {
            // External bitcoind requires credentials (unless connecting to localhost without auth)
            // This is a warning-worthy case but we allow it for flexibility
        }

        // Validate GPU batch exponent range
        if self.mining.gpu_batch_exp < 20 || self.mining.gpu_batch_exp > 28 {
            return Err(ConfigError::ValidationError(
                "gpu_batch_exp must be between 20 and 28".to_string(),
            ));
        }

        // Validate GPU intensity range
        if self.mining.gpu_intensity == 0 || self.mining.gpu_intensity > 100 {
            return Err(ConfigError::ValidationError(
                "gpu_intensity must be between 1 and 100".to_string(),
            ));
        }

        // Validate web auth config
        if self.web.auth_enabled
            && (self.web.auth_user.is_none() || self.web.auth_password.is_none())
        {
            return Err(ConfigError::ValidationError(
                "web.auth_user and web.auth_password must be set when web.auth_enabled=true"
                    .to_string(),
            ));
        }

        // Validate port ranges (web.port 0 means disabled, others must be > 0)
        // Note: u16 max is 65535, so no upper bound check needed
        if self.bitcoind.rpc_port == 0 {
            return Err(ConfigError::ValidationError(
                "bitcoind.rpc_port must be between 1 and 65535".to_string(),
            ));
        }

        if self.sv2.proxy_port == 0 {
            return Err(ConfigError::ValidationError(
                "sv2.proxy_port must be between 1 and 65535".to_string(),
            ));
        }

        // Validate reconnect delay consistency
        if self.network.max_reconnect_delay < self.network.reconnect_delay {
            return Err(ConfigError::ValidationError(
                "network.max_reconnect_delay must be >= network.reconnect_delay".to_string(),
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
        assert!(idle.cpu);
        assert!(idle.gpu);
        assert!(!idle.gpu_inverse);
        assert!(idle.gpu_devices.is_empty());
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
        // Defaults should be applied
        assert!(config.idle.cpu);
        assert!(config.idle.gpu);
    }

    #[test]
    fn test_idle_config_cpu_gpu_selection() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [idle]
            enabled = true
            cpu = false
            gpu = true
            gpu_devices = [0, 1]
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.idle.enabled);
        assert!(!config.idle.cpu);
        assert!(config.idle.gpu);
        assert!(!config.idle.gpu_inverse); // default
        assert_eq!(config.idle.gpu_devices, vec![0, 1]);
    }

    #[test]
    fn test_idle_config_gpu_inverse() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [idle]
            enabled = true
            cpu = true
            gpu = true
            gpu_inverse = true
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.idle.enabled);
        assert!(config.idle.cpu);
        assert!(config.idle.gpu);
        assert!(config.idle.gpu_inverse);
    }

    #[test]
    fn test_idle_config_gpu_inverse_default_false() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [idle]
            enabled = true
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        // gpu_inverse should default to false
        assert!(!config.idle.gpu_inverse);
    }

    #[test]
    fn test_pool_preset_stratum_version() {
        assert_eq!(PoolPreset::Ckpool.stratum_version(), StratumVersion::V1);
        assert_eq!(PoolPreset::Braiins.stratum_version(), StratumVersion::V1);
        assert_eq!(PoolPreset::Ocean.stratum_version(), StratumVersion::V1);
        assert_eq!(PoolPreset::None.stratum_version(), StratumVersion::V1);
        assert_eq!(PoolPreset::Custom.stratum_version(), StratumVersion::V1);
    }

    #[test]
    fn test_pool_preset_requires_bitcoind() {
        assert!(!PoolPreset::Ckpool.requires_bitcoind());
        assert!(!PoolPreset::Braiins.requires_bitcoind());
        assert!(!PoolPreset::Ocean.requires_bitcoind());
        assert!(PoolPreset::None.requires_bitcoind());
        assert!(!PoolPreset::Custom.requires_bitcoind());
    }

    #[test]
    fn test_pool_preset_custom_address() {
        assert_eq!(PoolPreset::Custom.address(), None);
    }

    #[test]
    fn test_pool_config_effective_address_custom() {
        let pool = PoolConfig {
            preset: PoolPreset::Custom,
            address: Some("custom.pool.com:3333".to_string()),
            payout_address: "bc1qtest".to_string(),
            password: "x".to_string(),
            stratum_version: None,
        };
        assert_eq!(pool.effective_address(), Some("custom.pool.com:3333".to_string()));
    }

    #[test]
    fn test_pool_config_effective_address_preset() {
        let pool = PoolConfig {
            preset: PoolPreset::Ckpool,
            address: None,
            payout_address: "bc1qtest".to_string(),
            password: "x".to_string(),
            stratum_version: None,
        };
        assert_eq!(pool.effective_address(), Some("solo.ckpool.org:3333".to_string()));
    }

    #[test]
    fn test_pool_config_effective_stratum_version_override() {
        let pool = PoolConfig {
            preset: PoolPreset::Ckpool,
            address: None,
            payout_address: "bc1qtest".to_string(),
            password: "x".to_string(),
            stratum_version: Some(StratumVersion::V2),
        };
        assert_eq!(pool.effective_stratum_version(), StratumVersion::V2);
    }

    #[test]
    fn test_config_validation_placeholder_address() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "YOUR_BTC_ADDRESS_HERE"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_custom_without_address() {
        let toml_content = r#"
            [pool]
            preset = "custom"
            payout_address = "bc1qtest"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_validation_no_mining_enabled() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            enable_cpu = false
            enable_gpu = false
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_config_is_solo_mode() {
        let toml_content = r#"
            [pool]
            preset = "none"
            payout_address = "bc1qtest"

            [bitcoind]
            enabled = true
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.is_solo_mode());
    }

    #[test]
    fn test_config_is_sv2_mode() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [sv2]
            enabled = true
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.is_sv2_mode());
    }

    #[test]
    fn test_config_is_sv2_mode_via_stratum_version() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"
            stratum_version = "v2"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.is_sv2_mode());
    }

    #[test]
    fn test_config_effective_threads_auto() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            threads = 0
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        // Should return number of CPUs
        assert!(config.effective_threads() > 0);
    }

    #[test]
    fn test_config_effective_threads_manual() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            threads = 4
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.effective_threads(), 4);
    }

    #[test]
    fn test_mining_config_default() {
        let mining = MiningConfig::default();
        assert!(mining.enable_cpu);
        assert_eq!(mining.threads, 0);
        assert!(!mining.enable_gpu);
        assert!(mining.gpu_devices.is_empty());
        assert_eq!(mining.gpu_intensity, 100);
        assert_eq!(mining.gpu_workgroup_size, 0);
        assert_eq!(mining.gpu_batch_exp, 25);
    }

    #[test]
    fn test_bitcoind_config_default() {
        let bitcoind = BitcoindConfig::default();
        assert!(!bitcoind.enabled);
        assert!(!bitcoind.managed);
        assert_eq!(bitcoind.host, "127.0.0.1");
        assert_eq!(bitcoind.rpc_port, 8332);
        assert_eq!(bitcoind.prune, 10000);
        assert_eq!(bitcoind.dbcache, 0);
        assert!(!bitcoind.assumeutxo);
        assert!(bitcoind.assumeutxo_magnet.is_none());
    }

    #[test]
    fn test_sv2_config_default() {
        let sv2 = Sv2Config::default();
        assert!(!sv2.enabled);
        assert_eq!(sv2.proxy_host, "127.0.0.1");
        assert_eq!(sv2.proxy_port, 34255);
    }

    #[test]
    fn test_web_config_default() {
        let web = WebConfig::default();
        assert_eq!(web.port, 8080);
        assert_eq!(web.dir, "web");
    }

    #[test]
    fn test_network_config_default() {
        let network = NetworkConfig::default();
        assert_eq!(network.reconnect_delay, 5);
        assert_eq!(network.max_reconnect_delay, 300);
    }

    #[test]
    fn test_logging_config_default() {
        let logging = LoggingConfig::default();
        assert_eq!(logging.stats_interval, 10);
        assert!(logging.colors);
        assert!(logging.file.is_none());
        assert_eq!(logging.level, "info");
        assert!(logging.system_bell);
        assert!(logging.hud);
        assert_eq!(logging.format, LogFormat::Text);
    }

    #[test]
    fn test_stratum_version_default() {
        let version = StratumVersion::default();
        assert_eq!(version, StratumVersion::V1);
    }

    #[test]
    fn test_pool_preset_default() {
        let preset = PoolPreset::default();
        assert_eq!(preset, PoolPreset::Ckpool);
    }

    #[test]
    fn test_mining_config_gpu_tuning_from_toml() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            enable_gpu = true
            gpu_intensity = 75
            gpu_workgroup_size = 512
            gpu_batch_exp = 26
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.mining.enable_gpu);
        assert_eq!(config.mining.gpu_intensity, 75);
        assert_eq!(config.mining.gpu_workgroup_size, 512);
        assert_eq!(config.mining.gpu_batch_exp, 26);
    }

    #[test]
    fn test_mining_config_gpu_tuning_defaults_in_toml() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            enable_gpu = true
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        // Should use defaults for unspecified GPU tuning options
        assert!(config.mining.enable_gpu);
        assert_eq!(config.mining.gpu_intensity, 100);  // default
        assert_eq!(config.mining.gpu_workgroup_size, 0);  // default
        assert_eq!(config.mining.gpu_batch_exp, 25);  // default
    }

    #[test]
    fn test_mining_config_gpu_devices_from_toml() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            enable_gpu = true
            gpu_devices = [0, 1, 2]
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.mining.gpu_devices, vec![0, 1, 2]);
    }

    #[test]
    fn test_mining_config_gpu_extreme_values() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            enable_gpu = true
            gpu_intensity = 1
            gpu_workgroup_size = 64
            gpu_batch_exp = 20
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.mining.gpu_intensity, 1);
        assert_eq!(config.mining.gpu_workgroup_size, 64);
        assert_eq!(config.mining.gpu_batch_exp, 20);
    }

    #[test]
    fn test_config_with_bitcoind_assumeutxo_magnet() {
        let toml_content = r#"
            [pool]
            preset = "none"
            payout_address = "bc1qtest"

            [bitcoind]
            enabled = true
            managed = true
            assumeutxo = true
            assumeutxo_magnet = "magnet:?xt=urn:btih:custom_hash"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.bitcoind.assumeutxo);
        assert_eq!(config.bitcoind.assumeutxo_magnet, Some("magnet:?xt=urn:btih:custom_hash".to_string()));
    }

    #[test]
    fn test_config_with_custom_pool_address() {
        let toml_content = r#"
            [pool]
            preset = "custom"
            address = "custom.pool.com:3333"
            payout_address = "bc1qtest"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.pool.preset, PoolPreset::Custom);
        assert_eq!(config.pool.address, Some("custom.pool.com:3333".to_string()));
        // Validation should pass with custom address
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_with_logging_file() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [logging]
            file = "miner.log"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.logging.file, Some("miner.log".to_string()));
    }

    #[test]
    fn test_config_with_rpc_credentials() {
        let toml_content = r#"
            [pool]
            preset = "none"
            payout_address = "bc1qtest"

            [bitcoind]
            enabled = true
            rpc_user = "myuser"
            rpc_password = "mypassword"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.bitcoind.rpc_user, Some("myuser".to_string()));
        assert_eq!(config.bitcoind.rpc_password, Some("mypassword".to_string()));
    }

    #[test]
    fn test_config_with_custom_datadir() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [bitcoind]
            enabled = false
            datadir = "/custom/bitcoin/path"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.bitcoind.datadir, Some("/custom/bitcoin/path".to_string()));
    }

    #[test]
    fn test_config_validation_rpc_port_zero() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [bitcoind]
            rpc_port = 0
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("rpc_port"));
    }

    #[test]
    fn test_config_validation_sv2_port_zero() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [sv2]
            proxy_port = 0
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("proxy_port"));
    }

    #[test]
    fn test_config_validation_reconnect_delay_invalid() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [network]
            reconnect_delay = 100
            max_reconnect_delay = 50
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("reconnect_delay"));
    }

    #[test]
    fn test_config_validation_gpu_batch_exp_too_low() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            gpu_batch_exp = 19
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("gpu_batch_exp"));
    }

    #[test]
    fn test_config_validation_gpu_batch_exp_too_high() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            gpu_batch_exp = 29
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("gpu_batch_exp"));
    }

    #[test]
    fn test_config_validation_gpu_intensity_zero() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            gpu_intensity = 0
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("gpu_intensity"));
    }

    #[test]
    fn test_config_validation_gpu_intensity_too_high() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            gpu_intensity = 101
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("gpu_intensity"));
    }

    #[test]
    fn test_config_validation_web_auth_missing_user() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [web]
            auth_enabled = true
            auth_password = "secret"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("auth_user"));
    }

    #[test]
    fn test_config_validation_web_auth_missing_password() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [web]
            auth_enabled = true
            auth_user = "admin"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("auth_password"));
    }

    #[test]
    fn test_config_validation_web_auth_valid() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [web]
            auth_enabled = true
            auth_user = "admin"
            auth_password = "secret123"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_managed_with_pool() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [bitcoind]
            enabled = true
            managed = true
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        let err = config.validate();
        assert!(err.is_err());
        assert!(format!("{:?}", err).contains("managed"));
    }

    #[test]
    fn test_web_port_zero_disabled() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [web]
            port = 0
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.web.port, 0);
        // Port 0 should be valid (means disabled)
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_simd_level_config_default() {
        let mining = MiningConfig::default();
        assert_eq!(mining.simd_level, SimdLevelConfig::Auto);
    }

    #[test]
    fn test_simd_level_config_from_toml() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            simd_level = "sha-ni"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.mining.simd_level, SimdLevelConfig::ShaNi);
    }

    #[test]
    fn test_simd_level_config_avx2() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            simd_level = "avx2"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.mining.simd_level, SimdLevelConfig::Avx2);
    }

    #[test]
    fn test_simd_level_config_avx512() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            simd_level = "avx512"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.mining.simd_level, SimdLevelConfig::Avx512);
    }

    #[test]
    fn test_simd_level_config_generic() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            simd_level = "generic"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.mining.simd_level, SimdLevelConfig::Generic);
    }

    #[test]
    fn test_simd_level_config_auto_explicit() {
        let toml_content = r#"
            [pool]
            preset = "ckpool"
            payout_address = "bc1qtest"

            [mining]
            simd_level = "auto"
        "#;
        let config: Config = toml::from_str(toml_content).unwrap();
        assert_eq!(config.mining.simd_level, SimdLevelConfig::Auto);
    }
}
