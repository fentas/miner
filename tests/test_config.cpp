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

void test_validation_braiins_v1_warning() {
    TEST_GROUP("Config Validation - Braiins V1 Warning");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.pool_host = "solo.braiins.com";
    cfg.stratum_version = "v1";  // Braiins supports V2

    auto result = config::validate(cfg);
    TEST_RESULT("Braiins with V1 generates warning", !result.warnings.empty());
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
    TEST_RESULT("Braiins host is correct", braiins.host == "solo.braiins.com");
    TEST_RESULT("Braiins uses V2", braiins.stratum_version == "v2");
}

void test_high_thread_warning() {
    TEST_GROUP("Config Validation - High Thread Count Warning");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.threads = 512;  // Unusually high

    auto result = config::validate(cfg);
    TEST_RESULT("High thread count generates warning", !result.warnings.empty());
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
    test_validation_braiins_v1_warning();
    test_validation_zero_port();
    test_pool_presets();
    test_high_thread_warning();
}

} // namespace test_config
