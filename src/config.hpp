#pragma once

#include <string>
#include <fstream>
#include <sstream>
#include <iostream>
#include <algorithm>
#include <map>
#include <vector>

namespace config {

// Solo pool presets
enum class SoloPool {
    Custom,     // User-defined pool
    CKPool,     // solo.ckpool.org (Stratum V1)
    Braiins,    // solo.stratum.braiins.com (Stratum V1)
    Ocean       // mine.ocean.xyz (Stratum V1 + DATUM)
};

// Pool preset info
struct PoolPreset {
    std::string host;
    uint16_t port;
    std::string stratum_version;
    std::string description;
};

inline const std::map<SoloPool, PoolPreset>& get_pool_presets() {
    static const std::map<SoloPool, PoolPreset> presets = {
        {SoloPool::CKPool,  {"solo.ckpool.org", 3333, "v1", "CKPool Solo (Stratum V1)"}},
        {SoloPool::Braiins, {"solo.stratum.braiins.com", 3333, "v1", "Braiins Solo (Stratum V1)"}},
        {SoloPool::Ocean,   {"mine.ocean.xyz", 3334, "v1", "Ocean Pool (Stratum V1 + DATUM)"}}
    };
    return presets;
}

struct Config {
    std::string pool_host = "solo.ckpool.org";
    uint16_t pool_port = 3333;
    std::string payout_address;
    std::string password = "x";
    unsigned threads = 0;
    unsigned stats_interval = 10;
    uint16_t web_port = 8080;
    std::string web_dir = "web";

    // Solo pool preset
    SoloPool solo_pool = SoloPool::CKPool;

    // Idle mining (screensaver mode)
    unsigned idle_timeout = 0;      // Seconds of inactivity before starting (0 = disabled)
    bool idle_mining_enabled = false;

    // Output options
    bool colors = true;             // Enable ANSI colors in output (default: on)
    std::string log_file;           // Log file path (empty = disabled)
    bool system_bell = true;        // System bell on share found

    // CPU mining
    bool enable_cpu = true;         // Enable CPU mining (default: on)

    // GPU mining
    bool enable_gpu = false;        // Enable GPU mining (default: off)

    // Stratum protocol: "v1" or "v2"
    std::string stratum_version = "v1";

    // Reconnection settings
    unsigned reconnect_delay = 5;   // Initial reconnect delay in seconds
    unsigned max_reconnect_delay = 300;  // Max reconnect delay (5 min)

    // Bitcoin Core integration (for Stratum V2 with custom block templates)
    bool bitcoind_enabled = false;          // Enable local Bitcoin Core integration
    bool bitcoind_managed = false;          // Auto download/install/start/stop Bitcoin Core
    std::string bitcoind_host = "127.0.0.1"; // Bitcoin Core RPC host
    uint16_t bitcoind_rpc_port = 8332;      // Bitcoin Core RPC port
    std::string bitcoind_rpc_user;          // RPC username
    std::string bitcoind_rpc_password;      // RPC password
    std::string bitcoind_datadir;           // Bitcoin Core data directory (optional)
    unsigned bitcoind_prune = 10000;        // Prune size in MB (0 = no pruning, 550 = minimum)
    bool bitcoind_sv2 = false;              // Enable Stratum V2 with Bitcoin Core (experimental)
};

// Validation result
struct ValidationResult {
    bool valid = true;
    std::vector<std::string> errors;
    std::vector<std::string> warnings;
};

inline std::string trim(const std::string& s) {
    size_t start = s.find_first_not_of(" \t\r\n");
    if (start == std::string::npos) return "";
    size_t end = s.find_last_not_of(" \t\r\n");
    return s.substr(start, end - start + 1);
}

inline bool load(const std::string& filename, Config& cfg) {
    std::ifstream file(filename);
    if (!file.is_open()) {
        return false;
    }

    std::string line;
    while (std::getline(file, line)) {
        // Skip comments and empty lines
        line = trim(line);
        if (line.empty() || line[0] == '#') continue;

        // Parse key = value
        size_t eq = line.find('=');
        if (eq == std::string::npos) continue;

        std::string key = trim(line.substr(0, eq));
        std::string value = trim(line.substr(eq + 1));

        // Convert key to lowercase for case-insensitive matching
        std::string key_lower = key;
        std::transform(key_lower.begin(), key_lower.end(), key_lower.begin(), ::tolower);

        if (key_lower == "pool") {
            size_t colon = value.rfind(':');
            if (colon != std::string::npos) {
                cfg.pool_host = value.substr(0, colon);
                cfg.pool_port = static_cast<uint16_t>(std::stoul(value.substr(colon + 1)));
            } else {
                cfg.pool_host = value;
            }
        } else if (key_lower == "payoutaddress" || key_lower == "address" || key_lower == "user") {
            cfg.payout_address = value;
        } else if (key_lower == "password" || key_lower == "pass") {
            cfg.password = value;
        } else if (key_lower == "threads") {
            cfg.threads = static_cast<unsigned>(std::stoul(value));
        } else if (key_lower == "statsinterval") {
            cfg.stats_interval = static_cast<unsigned>(std::stoul(value));
        } else if (key_lower == "webport") {
            cfg.web_port = static_cast<uint16_t>(std::stoul(value));
        } else if (key_lower == "webdir") {
            cfg.web_dir = value;
        } else if (key_lower == "idletimeout" || key_lower == "idle_timeout") {
            cfg.idle_timeout = static_cast<unsigned>(std::stoul(value));
            cfg.idle_mining_enabled = cfg.idle_timeout > 0;
        } else if (key_lower == "idlemining" || key_lower == "idle_mining") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.idle_mining_enabled = (val_lower == "true" || val_lower == "1" || val_lower == "yes");
        } else if (key_lower == "colors" || key_lower == "color") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.colors = (val_lower == "true" || val_lower == "1" || val_lower == "yes" || val_lower == "on");
        } else if (key_lower == "logfile" || key_lower == "log_file" || key_lower == "log") {
            cfg.log_file = value;
        } else if (key_lower == "systembell" || key_lower == "system_bell" || key_lower == "bell") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.system_bell = (val_lower == "true" || val_lower == "1" || val_lower == "yes" || val_lower == "on");
        } else if (key_lower == "enablecpu" || key_lower == "enable_cpu" || key_lower == "cpu") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.enable_cpu = (val_lower == "true" || val_lower == "1" || val_lower == "yes" || val_lower == "on");
        } else if (key_lower == "enablegpu" || key_lower == "enable_gpu" || key_lower == "gpu") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.enable_gpu = (val_lower == "true" || val_lower == "1" || val_lower == "yes" || val_lower == "on");
        } else if (key_lower == "stratumversion" || key_lower == "stratum_version" || key_lower == "stratum") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.stratum_version = val_lower;  // "v1" or "v2"
        } else if (key_lower == "solopool" || key_lower == "solo_pool" || key_lower == "poolpreset" || key_lower == "pool_preset") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            if (val_lower == "ckpool" || val_lower == "ck" || val_lower == "v1") {
                cfg.solo_pool = SoloPool::CKPool;
                // Apply preset
                const auto& preset = get_pool_presets().at(SoloPool::CKPool);
                cfg.pool_host = preset.host;
                cfg.pool_port = preset.port;
                cfg.stratum_version = preset.stratum_version;
            } else if (val_lower == "braiins" || val_lower == "slush" || val_lower == "v2") {
                cfg.solo_pool = SoloPool::Braiins;
                // Apply preset
                const auto& preset = get_pool_presets().at(SoloPool::Braiins);
                cfg.pool_host = preset.host;
                cfg.pool_port = preset.port;
                cfg.stratum_version = preset.stratum_version;
            } else if (val_lower == "custom") {
                cfg.solo_pool = SoloPool::Custom;
            }
        } else if (key_lower == "reconnectdelay" || key_lower == "reconnect_delay") {
            cfg.reconnect_delay = static_cast<unsigned>(std::stoul(value));
        } else if (key_lower == "maxreconnectdelay" || key_lower == "max_reconnect_delay") {
            cfg.max_reconnect_delay = static_cast<unsigned>(std::stoul(value));
        }
        // Bitcoin Core integration
        else if (key_lower == "bitcoindenabled" || key_lower == "bitcoind_enabled" || key_lower == "bitcoind") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.bitcoind_enabled = (val_lower == "true" || val_lower == "1" || val_lower == "yes" || val_lower == "on");
        } else if (key_lower == "bitcoindmanaged" || key_lower == "bitcoind_managed") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.bitcoind_managed = (val_lower == "true" || val_lower == "1" || val_lower == "yes" || val_lower == "on");
        } else if (key_lower == "bitcoindhost" || key_lower == "bitcoind_host" || key_lower == "rpchost") {
            cfg.bitcoind_host = value;
        } else if (key_lower == "bitcoindrpcport" || key_lower == "bitcoind_rpc_port" || key_lower == "rpcport") {
            cfg.bitcoind_rpc_port = static_cast<uint16_t>(std::stoul(value));
        } else if (key_lower == "bitcoindrpcuser" || key_lower == "bitcoind_rpc_user" || key_lower == "rpcuser") {
            cfg.bitcoind_rpc_user = value;
        } else if (key_lower == "bitcoindrpcpassword" || key_lower == "bitcoind_rpc_password" || key_lower == "rpcpassword") {
            cfg.bitcoind_rpc_password = value;
        } else if (key_lower == "bitcoinddatadir" || key_lower == "bitcoind_datadir" || key_lower == "datadir") {
            cfg.bitcoind_datadir = value;
        } else if (key_lower == "bitcoindprune" || key_lower == "bitcoind_prune" || key_lower == "prune") {
            cfg.bitcoind_prune = static_cast<unsigned>(std::stoul(value));
        } else if (key_lower == "bitcoindsv2" || key_lower == "bitcoind_sv2") {
            std::string val_lower = value;
            std::transform(val_lower.begin(), val_lower.end(), val_lower.begin(), ::tolower);
            cfg.bitcoind_sv2 = (val_lower == "true" || val_lower == "1" || val_lower == "yes" || val_lower == "on");
        }
    }

    return true;
}

// Validate configuration and return errors/warnings
inline ValidationResult validate(const Config& cfg) {
    ValidationResult result;

    // Required: PayoutAddress
    if (cfg.payout_address.empty()) {
        result.valid = false;
        result.errors.push_back("PayoutAddress is required");
    } else {
        // Basic Bitcoin address validation
        if (cfg.payout_address.length() < 26 || cfg.payout_address.length() > 90) {
            result.valid = false;
            result.errors.push_back("PayoutAddress appears invalid (wrong length)");
        } else if (cfg.payout_address[0] != '1' && cfg.payout_address[0] != '3' &&
                   cfg.payout_address.substr(0, 3) != "bc1") {
            result.warnings.push_back("PayoutAddress doesn't look like a valid Bitcoin address");
        }
    }

    // Mining mode validation - at least one of CPU or GPU must be enabled
    if (!cfg.enable_cpu && !cfg.enable_gpu) {
        result.valid = false;
        result.errors.push_back("At least one mining mode must be enabled (EnableCPU or EnableGPU)");
    }

    // Stratum version validation
    if (cfg.stratum_version != "v1" && cfg.stratum_version != "v2") {
        result.valid = false;
        result.errors.push_back("Stratum version must be 'v1' or 'v2', got: " + cfg.stratum_version);
    }

    // Pool/Stratum compatibility warnings
    // Note: Both CKPool and Braiins Solo use Stratum V1 (CKPool software stack)
    bool is_ckpool = (cfg.pool_host.find("ckpool") != std::string::npos);
    bool is_braiins_solo = (cfg.pool_host.find("solo.stratum.braiins") != std::string::npos);

    if (cfg.stratum_version == "v2" && (is_ckpool || is_braiins_solo)) {
        result.warnings.push_back("This pool uses Stratum V1. Change Stratum to 'v1'");
    }

    // Thread validation
    if (cfg.threads > 256) {
        result.warnings.push_back("Thread count (" + std::to_string(cfg.threads) + ") is unusually high");
    }

    // Port validation
    if (cfg.pool_port == 0) {
        result.valid = false;
        result.errors.push_back("Pool port cannot be 0");
    }

    // Stats interval
    if (cfg.stats_interval == 0) {
        result.warnings.push_back("StatsInterval is 0, stats will not be displayed");
    } else if (cfg.stats_interval < 5) {
        result.warnings.push_back("StatsInterval is very low (" + std::to_string(cfg.stats_interval) + "s), may cause excessive output");
    }

    // Reconnect delay
    if (cfg.reconnect_delay == 0) {
        result.warnings.push_back("ReconnectDelay is 0, will reconnect immediately on disconnect");
    }

    if (cfg.max_reconnect_delay < cfg.reconnect_delay) {
        result.warnings.push_back("MaxReconnectDelay is less than ReconnectDelay");
    }

    // Bitcoin Core validation
    if (cfg.bitcoind_enabled) {
        if (cfg.bitcoind_rpc_user.empty()) {
            result.warnings.push_back("BitcoindRpcUser not set - RPC authentication may fail");
        }
        if (cfg.bitcoind_rpc_password.empty()) {
            result.warnings.push_back("BitcoindRpcPassword not set - RPC authentication may fail");
        }
        if (cfg.bitcoind_prune > 0 && cfg.bitcoind_prune < 550) {
            result.warnings.push_back("BitcoindPrune minimum is 550 MB, got " + std::to_string(cfg.bitcoind_prune));
        }
        if (cfg.bitcoind_sv2) {
            result.warnings.push_back("Stratum V2 with Bitcoin Core is experimental (requires Bitcoin Core v30+)");
        }
    }

    return result;
}

// Print validation results
inline void print_validation(const ValidationResult& result) {
    for (const auto& error : result.errors) {
        std::cerr << "[Config] ERROR: " << error << std::endl;
    }
    for (const auto& warning : result.warnings) {
        std::cerr << "[Config] WARNING: " << warning << std::endl;
    }
}

inline void print(const Config& cfg) {
    std::cout << "[Config] Pool: " << cfg.pool_host << ":" << cfg.pool_port << "\n"
              << "[Config] Stratum: " << cfg.stratum_version << "\n"
              << "[Config] Address: " << cfg.payout_address << "\n"
              << "[Config] Threads: " << (cfg.threads == 0 ? "auto" : std::to_string(cfg.threads)) << "\n"
              << "[Config] Stats interval: " << cfg.stats_interval << "s\n"
              << std::endl;
}

} // namespace config
