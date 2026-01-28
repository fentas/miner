// Config Validation Tests

#include "test_framework.hpp"
#include "../src/config.hpp"

namespace test_config {

void test_validation_valid_config() {
    TEST_GROUP("Config Validation - Valid Config");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.pool_host = "solo.ckpool.org";
    cfg.pool_port = 3333;
    cfg.stratum_version = "v1";

    auto result = config::validate(cfg);
    TEST_RESULT("Valid config passes validation", result.valid);
    TEST_RESULT("No errors on valid config", result.errors.empty());
}

void test_validation_missing_address() {
    TEST_GROUP("Config Validation - Missing Address");

    config::Config cfg;
    cfg.payout_address = "";

    auto result = config::validate(cfg);
    TEST_RESULT("Missing address fails validation", !result.valid);
    TEST_RESULT("Has error for missing address", !result.errors.empty());
}

void test_validation_invalid_address_length() {
    TEST_GROUP("Config Validation - Invalid Address Length");

    config::Config cfg;
    cfg.payout_address = "abc";  // Too short

    auto result = config::validate(cfg);
    TEST_RESULT("Short address fails validation", !result.valid);
}

void test_validation_legacy_address() {
    TEST_GROUP("Config Validation - Legacy Address");

    config::Config cfg;
    cfg.payout_address = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";  // Satoshi's address
    cfg.stratum_version = "v1";

    auto result = config::validate(cfg);
    TEST_RESULT("Legacy address (1...) is valid", result.valid);
}

void test_validation_p2sh_address() {
    TEST_GROUP("Config Validation - P2SH Address");

    config::Config cfg;
    cfg.payout_address = "3J98t1WpEZ73CNmQviecrnyiWrnqRhWNLy";
    cfg.stratum_version = "v1";

    auto result = config::validate(cfg);
    TEST_RESULT("P2SH address (3...) is valid", result.valid);
}

void test_validation_bech32_address() {
    TEST_GROUP("Config Validation - Bech32 Address");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.stratum_version = "v1";

    auto result = config::validate(cfg);
    TEST_RESULT("Bech32 address (bc1...) is valid", result.valid);
}

void test_validation_invalid_stratum_version() {
    TEST_GROUP("Config Validation - Invalid Stratum Version");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.stratum_version = "v3";  // Invalid

    auto result = config::validate(cfg);
    TEST_RESULT("Invalid stratum version fails", !result.valid);
}

void test_validation_ckpool_v2_warning() {
    TEST_GROUP("Config Validation - CKPool V2 Warning");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.pool_host = "solo.ckpool.org";
    cfg.stratum_version = "v2";  // CKPool doesn't support V2

    auto result = config::validate(cfg);
    TEST_RESULT("CKPool with V2 generates warning", !result.warnings.empty());
}

void test_validation_braiins_solo_v1() {
    TEST_GROUP("Config Validation - Braiins Solo V1");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.pool_host = "solo.stratum.braiins.com";
    cfg.stratum_version = "v1";  // Braiins Solo uses V1 (CKPool stack)

    auto result = config::validate(cfg);
    TEST_RESULT("Braiins Solo with V1 is valid", result.valid);
}

void test_validation_zero_port() {
    TEST_GROUP("Config Validation - Zero Port");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.pool_port = 0;

    auto result = config::validate(cfg);
    TEST_RESULT("Zero port fails validation", !result.valid);
}

void test_pool_presets() {
    TEST_GROUP("Pool Presets");

    const auto& presets = config::get_pool_presets();

    TEST_RESULT("CKPool preset exists", presets.count(config::SoloPool::CKPool) == 1);
    TEST_RESULT("Braiins preset exists", presets.count(config::SoloPool::Braiins) == 1);

    const auto& ckpool = presets.at(config::SoloPool::CKPool);
    TEST_RESULT("CKPool host is correct", ckpool.host == "solo.ckpool.org");
    TEST_RESULT("CKPool uses V1", ckpool.stratum_version == "v1");

    const auto& braiins = presets.at(config::SoloPool::Braiins);
    TEST_RESULT("Braiins host is correct", braiins.host == "solo.stratum.braiins.com");
    TEST_RESULT("Braiins uses V1", braiins.stratum_version == "v1");
}

void test_high_thread_warning() {
    TEST_GROUP("Config Validation - High Thread Count Warning");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.threads = 512;  // Unusually high

    auto result = config::validate(cfg);
    TEST_RESULT("High thread count generates warning", !result.warnings.empty());
}

void test_mining_mode_cpu_only() {
    TEST_GROUP("Config Validation - CPU Only Mode");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.enable_cpu = true;
    cfg.enable_gpu = false;

    auto result = config::validate(cfg);
    TEST_RESULT("CPU-only mode is valid", result.valid);
}

void test_mining_mode_gpu_only() {
    TEST_GROUP("Config Validation - GPU Only Mode");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.enable_cpu = false;
    cfg.enable_gpu = true;

    auto result = config::validate(cfg);
    TEST_RESULT("GPU-only mode is valid", result.valid);
}

void test_mining_mode_both() {
    TEST_GROUP("Config Validation - CPU + GPU Mode");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.enable_cpu = true;
    cfg.enable_gpu = true;

    auto result = config::validate(cfg);
    TEST_RESULT("CPU + GPU mode is valid", result.valid);
}

void test_mining_mode_none_disabled() {
    TEST_GROUP("Config Validation - No Mining Mode Error");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.enable_cpu = false;
    cfg.enable_gpu = false;

    auto result = config::validate(cfg);
    TEST_RESULT("No mining mode fails validation", !result.valid);
    TEST_RESULT("Error mentions mining mode", !result.errors.empty());
}

// ===========================================
// True Solo Mode (SoloPool::None) Tests
// ===========================================

void test_solo_mode_valid_config() {
    TEST_GROUP("Config Validation - Solo Mode Valid");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.solo_pool = config::SoloPool::None;
    cfg.pool_host = "";
    cfg.pool_port = 0;
    cfg.bitcoind_enabled = true;
    cfg.bitcoind_managed = true;

    auto result = config::validate(cfg);
    TEST_RESULT("Solo mode with bitcoind enabled is valid", result.valid);
}

void test_solo_mode_requires_bitcoind() {
    TEST_GROUP("Config Validation - Solo Mode Requires Bitcoind");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.solo_pool = config::SoloPool::None;
    cfg.pool_host = "";
    cfg.pool_port = 0;
    cfg.bitcoind_enabled = false;  // Missing required bitcoind

    auto result = config::validate(cfg);
    TEST_RESULT("Solo mode without bitcoind fails validation", !result.valid);

    bool has_bitcoind_error = false;
    for (const auto& e : result.errors) {
        if (e.find("BitcoindEnabled") != std::string::npos) {
            has_bitcoind_error = true;
        }
    }
    TEST_RESULT("Error mentions BitcoindEnabled requirement", has_bitcoind_error);
}

void test_solo_mode_zero_port_allowed() {
    TEST_GROUP("Config Validation - Solo Mode Zero Port Allowed");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.solo_pool = config::SoloPool::None;
    cfg.pool_host = "";
    cfg.pool_port = 0;  // Zero port is OK for solo mode
    cfg.bitcoind_enabled = true;

    auto result = config::validate(cfg);

    // Should not have error about port (that's only for pool mode)
    bool has_port_error = false;
    for (const auto& e : result.errors) {
        if (e.find("port") != std::string::npos || e.find("Port") != std::string::npos) {
            has_port_error = true;
        }
    }
    TEST_RESULT("Solo mode accepts zero port", !has_port_error);
}

void test_solo_mode_has_warnings() {
    TEST_GROUP("Config Validation - Solo Mode Warnings");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.solo_pool = config::SoloPool::None;
    cfg.bitcoind_enabled = true;

    auto result = config::validate(cfg);

    bool has_solo_warning = false;
    for (const auto& w : result.warnings) {
        if (w.find("solo") != std::string::npos || w.find("Solo") != std::string::npos) {
            has_solo_warning = true;
        }
    }
    TEST_RESULT("Solo mode has informational warnings", has_solo_warning);
}

void test_solo_mode_config_parsing() {
    TEST_GROUP("Config Parsing - Solo Mode");

    // Create temp config file
    std::string tmp_conf = "/tmp/test_solo_mode.conf";
    {
        std::ofstream f(tmp_conf);
        f << "SoloPool = none\n"
          << "PayoutAddress = bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh\n"
          << "BitcoindEnabled = true\n"
          << "BitcoindManaged = true\n";
    }

    config::Config cfg;
    bool loaded = config::load(tmp_conf, cfg);

    TEST_RESULT("Config file loaded successfully", loaded);
    TEST_RESULT("SoloPool parsed as None", cfg.solo_pool == config::SoloPool::None);
    TEST_RESULT("Pool host is empty for solo mode", cfg.pool_host.empty());
    TEST_RESULT("Pool port is 0 for solo mode", cfg.pool_port == 0);
    TEST_RESULT("BitcoindEnabled auto-enabled", cfg.bitcoind_enabled == true);

    std::remove(tmp_conf.c_str());
}

void test_solo_mode_config_aliases() {
    TEST_GROUP("Config Parsing - Solo Mode Aliases");

    // Test "solo" alias
    std::string tmp_conf = "/tmp/test_solo_alias.conf";
    {
        std::ofstream f(tmp_conf);
        f << "SoloPool = solo\n"
          << "PayoutAddress = bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh\n";
    }

    config::Config cfg;
    config::load(tmp_conf, cfg);
    TEST_RESULT("'solo' alias maps to SoloPool::None", cfg.solo_pool == config::SoloPool::None);

    // Test "local" alias
    {
        std::ofstream f(tmp_conf);
        f << "SoloPool = local\n"
          << "PayoutAddress = bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh\n";
    }

    config::Config cfg2;
    config::load(tmp_conf, cfg2);
    TEST_RESULT("'local' alias maps to SoloPool::None", cfg2.solo_pool == config::SoloPool::None);

    std::remove(tmp_conf.c_str());
}

void test_solo_pool_none_enum() {
    TEST_GROUP("SoloPool Enum - None Value");

    TEST_RESULT("SoloPool::None exists", static_cast<int>(config::SoloPool::None) >= 0);
    TEST_RESULT("SoloPool::None is different from Custom",
                config::SoloPool::None != config::SoloPool::Custom);
    TEST_RESULT("SoloPool::None is different from CKPool",
                config::SoloPool::None != config::SoloPool::CKPool);
}

void run_all() {
    test_validation_valid_config();
    test_validation_missing_address();
    test_validation_invalid_address_length();
    test_validation_legacy_address();
    test_validation_p2sh_address();
    test_validation_bech32_address();
    test_validation_invalid_stratum_version();
    test_validation_ckpool_v2_warning();
    test_validation_braiins_solo_v1();
    test_validation_zero_port();
    test_pool_presets();
    test_high_thread_warning();
    test_mining_mode_cpu_only();
    test_mining_mode_gpu_only();
    test_mining_mode_both();
    test_mining_mode_none_disabled();

    // True Solo Mode (SoloPool::None) tests
    test_solo_mode_valid_config();
    test_solo_mode_requires_bitcoind();
    test_solo_mode_zero_port_allowed();
    test_solo_mode_has_warnings();
    test_solo_mode_config_parsing();
    test_solo_mode_config_aliases();
    test_solo_pool_none_enum();
}

} // namespace test_config
