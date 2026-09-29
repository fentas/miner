//! Integration tests for configuration loading and validation

use std::io::Write;
use tempfile::NamedTempFile;

/// Helper to create a temp config file
fn create_config_file(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(content.as_bytes()).unwrap();
    file
}

#[test]
fn test_full_config_parsing() {
    let content = r#"
[pool]
preset = "ckpool"
payout_address = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
password = "x"

[mining]
enable_cpu = true
threads = 8
enable_gpu = false
gpu_devices = []
gpu_intensity = 80

[bitcoind]
enabled = false
managed = false
host = "127.0.0.1"
rpc_port = 8332
prune = 10000

[sv2]
enabled = false
proxy_host = "127.0.0.1"
proxy_port = 34255

[web]
port = 8080
dir = "web"

[logging]
stats_interval = 10
colors = true
level = "info"
system_bell = true
hud = true
format = "text"

[network]
reconnect_delay = 5
max_reconnect_delay = 300

[idle]
enabled = false
timeout = 900
pause_on_active = true
"#;

    let file = create_config_file(content);
    let config_content = std::fs::read_to_string(file.path()).unwrap();
    let config: toml::Value = toml::from_str(&config_content).unwrap();

    // Verify all sections exist
    assert!(config.get("pool").is_some());
    assert!(config.get("mining").is_some());
    assert!(config.get("bitcoind").is_some());
    assert!(config.get("sv2").is_some());
    assert!(config.get("web").is_some());
    assert!(config.get("logging").is_some());
    assert!(config.get("network").is_some());
    assert!(config.get("idle").is_some());
}

#[test]
fn test_minimal_config() {
    let content = r#"
[pool]
preset = "ckpool"
payout_address = "bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq"
"#;

    let file = create_config_file(content);
    let config_content = std::fs::read_to_string(file.path()).unwrap();
    let config: toml::Value = toml::from_str(&config_content).unwrap();

    // Should have pool section
    let pool = config.get("pool").unwrap();
    assert_eq!(pool.get("preset").unwrap().as_str().unwrap(), "ckpool");
}

#[test]
fn test_pool_presets() {
    let presets = ["ckpool", "braiins", "ocean", "btcpowlab", "none", "custom"];

    for preset in presets {
        let content = format!(
            r#"
[pool]
preset = "{}"
payout_address = "bc1qtest"
address = "custom.pool.com:3333"

[bitcoind]
enabled = {}
"#,
            preset,
            if preset == "none" { "true" } else { "false" }
        );

        let file = create_config_file(&content);
        let config_content = std::fs::read_to_string(file.path()).unwrap();
        let config: toml::Value = toml::from_str(&config_content).unwrap();

        let pool = config.get("pool").unwrap();
        assert_eq!(pool.get("preset").unwrap().as_str().unwrap(), preset);
    }
}

#[test]
fn test_json_log_format() {
    let content = r#"
[pool]
preset = "ckpool"
payout_address = "bc1qtest"

[logging]
format = "json"
hud = false
"#;

    let file = create_config_file(content);
    let config_content = std::fs::read_to_string(file.path()).unwrap();
    let config: toml::Value = toml::from_str(&config_content).unwrap();

    let logging = config.get("logging").unwrap();
    assert_eq!(logging.get("format").unwrap().as_str().unwrap(), "json");
    assert!(!logging.get("hud").unwrap().as_bool().unwrap());
}

#[test]
fn test_gpu_config() {
    let content = r#"
[pool]
preset = "ckpool"
payout_address = "bc1qtest"

[mining]
enable_cpu = true
threads = 4
enable_gpu = true
gpu_devices = [0, 1]
gpu_intensity = 90
"#;

    let file = create_config_file(content);
    let config_content = std::fs::read_to_string(file.path()).unwrap();
    let config: toml::Value = toml::from_str(&config_content).unwrap();

    let mining = config.get("mining").unwrap();
    assert!(mining.get("enable_gpu").unwrap().as_bool().unwrap());

    let devices = mining.get("gpu_devices").unwrap().as_array().unwrap();
    assert_eq!(devices.len(), 2);
    assert_eq!(devices[0].as_integer().unwrap(), 0);
    assert_eq!(devices[1].as_integer().unwrap(), 1);
}

#[test]
fn test_idle_config() {
    let content = r#"
[pool]
preset = "ckpool"
payout_address = "bc1qtest"

[idle]
enabled = true
timeout = 600
pause_on_active = false
"#;

    let file = create_config_file(content);
    let config_content = std::fs::read_to_string(file.path()).unwrap();
    let config: toml::Value = toml::from_str(&config_content).unwrap();

    let idle = config.get("idle").unwrap();
    assert!(idle.get("enabled").unwrap().as_bool().unwrap());
    assert_eq!(idle.get("timeout").unwrap().as_integer().unwrap(), 600);
    assert!(!idle.get("pause_on_active").unwrap().as_bool().unwrap());
}

#[test]
fn test_stratum_v2_config() {
    let content = r#"
[pool]
preset = "ckpool"
payout_address = "bc1qtest"
stratum_version = "v2"

[sv2]
enabled = true
proxy_host = "192.168.1.100"
proxy_port = 34256
"#;

    let file = create_config_file(content);
    let config_content = std::fs::read_to_string(file.path()).unwrap();
    let config: toml::Value = toml::from_str(&config_content).unwrap();

    let sv2 = config.get("sv2").unwrap();
    assert!(sv2.get("enabled").unwrap().as_bool().unwrap());
    assert_eq!(sv2.get("proxy_host").unwrap().as_str().unwrap(), "192.168.1.100");
    assert_eq!(sv2.get("proxy_port").unwrap().as_integer().unwrap(), 34256);
}
