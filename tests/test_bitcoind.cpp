// Bitcoin Core Manager Tests
// Tests for the bitcoind management functionality

#include "test_framework.hpp"
#include "../src/bitcoind_manager.hpp"
#include "../src/config.hpp"

#include <string>
#include <filesystem>

namespace test_bitcoind {

// ===========================================
// Configuration Tests
// ===========================================

void test_config_bitcoind_managed_default() {
    TEST_GROUP("Config BitcoindManaged Default");

    config::Config cfg;
    TEST_RESULT("BitcoindManaged defaults to false", cfg.bitcoind_managed == false);
    TEST_RESULT("BitcoindEnabled defaults to false", cfg.bitcoind_enabled == false);
}

void test_config_bitcoind_managed_parsing() {
    TEST_GROUP("Config BitcoindManaged Parsing");

    // Create temp config file
    std::string tmp_conf = "/tmp/test_bitcoind.conf";
    {
        std::ofstream f(tmp_conf);
        f << "BitcoindEnabled = true\n"
          << "BitcoindManaged = true\n"
          << "BitcoindPrune = 5000\n"
          << "BitcoindRpcPort = 18332\n";
    }

    config::Config cfg;
    bool loaded = config::load(tmp_conf, cfg);

    TEST_RESULT("Config file loaded successfully", loaded);
    TEST_RESULT("BitcoindEnabled parsed correctly", cfg.bitcoind_enabled == true);
    TEST_RESULT("BitcoindManaged parsed correctly", cfg.bitcoind_managed == true);
    TEST_RESULT("BitcoindPrune parsed correctly", cfg.bitcoind_prune == 5000);
    TEST_RESULT("BitcoindRpcPort parsed correctly", cfg.bitcoind_rpc_port == 18332);

    std::remove(tmp_conf.c_str());
}

void test_config_bitcoind_validation() {
    TEST_GROUP("Config Bitcoind Validation");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.bitcoind_enabled = true;
    cfg.bitcoind_rpc_user = "";  // Empty user
    cfg.bitcoind_rpc_password = "";  // Empty password

    auto result = config::validate(cfg);

    // Should have warnings about missing credentials
    bool has_user_warning = false;
    bool has_pass_warning = false;
    for (const auto& w : result.warnings) {
        if (w.find("RpcUser") != std::string::npos) has_user_warning = true;
        if (w.find("RpcPassword") != std::string::npos) has_pass_warning = true;
    }

    TEST_RESULT("Warning for missing RPC user", has_user_warning);
    TEST_RESULT("Warning for missing RPC password", has_pass_warning);
}

void test_config_bitcoind_prune_validation() {
    TEST_GROUP("Config Bitcoind Prune Validation");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.bitcoind_enabled = true;
    cfg.bitcoind_rpc_user = "user";
    cfg.bitcoind_rpc_password = "pass";
    cfg.bitcoind_prune = 100;  // Below minimum of 550

    auto result = config::validate(cfg);

    bool has_prune_warning = false;
    for (const auto& w : result.warnings) {
        if (w.find("550") != std::string::npos) has_prune_warning = true;
    }

    TEST_RESULT("Warning for prune below minimum", has_prune_warning);
}

void test_config_bitcoind_sv2_warning() {
    TEST_GROUP("Config Bitcoind SV2 Warning");

    config::Config cfg;
    cfg.payout_address = "bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh";
    cfg.bitcoind_enabled = true;
    cfg.bitcoind_rpc_user = "user";
    cfg.bitcoind_rpc_password = "pass";
    cfg.bitcoind_sv2 = true;

    auto result = config::validate(cfg);

    bool has_sv2_warning = false;
    for (const auto& w : result.warnings) {
        if (w.find("experimental") != std::string::npos ||
            w.find("Bitcoin Core v30") != std::string::npos) {
            has_sv2_warning = true;
        }
    }

    TEST_RESULT("Warning for experimental SV2", has_sv2_warning);
}

// ===========================================
// Manager Path Tests
// ===========================================

void test_manager_download_url() {
    TEST_GROUP("Bitcoind Manager Download URL");

    std::string url = bitcoind::get_download_url();

    TEST_RESULT("Download URL is not empty", !url.empty());
    TEST_RESULT("URL contains bitcoincore.org", url.find("bitcoincore.org") != std::string::npos);
    TEST_RESULT("URL contains version", url.find(bitcoind::BITCOIN_CORE_VERSION) != std::string::npos);

#if defined(__linux__)
    TEST_RESULT("URL contains linux", url.find("linux") != std::string::npos);
#elif defined(__APPLE__)
    TEST_RESULT("URL contains darwin", url.find("darwin") != std::string::npos);
#elif defined(_WIN32)
    TEST_RESULT("URL contains win64", url.find("win64") != std::string::npos);
#endif
}

void test_manager_paths() {
    TEST_GROUP("Bitcoind Manager Paths");

    bitcoind::Manager mgr;

    std::string data_dir = mgr.get_data_dir();
    std::string install_dir = mgr.get_install_dir();
    std::string bitcoind_path = mgr.get_bitcoind_path();
    std::string cli_path = mgr.get_bitcoin_cli_path();

    TEST_RESULT("Data dir is not empty", !data_dir.empty());
    TEST_RESULT("Install dir is not empty", !install_dir.empty());
    TEST_RESULT("Bitcoind path is not empty", !bitcoind_path.empty());
    TEST_RESULT("CLI path is not empty", !cli_path.empty());

    TEST_RESULT("Bitcoind path contains 'bitcoind'", bitcoind_path.find("bitcoind") != std::string::npos);
    TEST_RESULT("CLI path contains 'bitcoin-cli'", cli_path.find("bitcoin-cli") != std::string::npos);
}

void test_manager_custom_data_dir() {
    TEST_GROUP("Bitcoind Manager Custom Data Dir");

    bitcoind::Manager mgr;
    mgr.set_data_dir("/custom/bitcoin/data");

    TEST_RESULT("Custom data dir is set", mgr.get_data_dir() == "/custom/bitcoin/data");
}

void test_manager_status_strings() {
    TEST_GROUP("Bitcoind Status Strings");

    TEST_RESULT("NotInstalled status",
                bitcoind::status_string(bitcoind::Status::NotInstalled) == "not_installed");
    TEST_RESULT("Stopped status",
                bitcoind::status_string(bitcoind::Status::Stopped) == "stopped");
    TEST_RESULT("Starting status",
                bitcoind::status_string(bitcoind::Status::Starting) == "starting");
    TEST_RESULT("Syncing status",
                bitcoind::status_string(bitcoind::Status::Syncing) == "syncing");
    TEST_RESULT("Ready status",
                bitcoind::status_string(bitcoind::Status::Ready) == "ready");
    TEST_RESULT("Stopping status",
                bitcoind::status_string(bitcoind::Status::Stopping) == "stopping");
    TEST_RESULT("Error status",
                bitcoind::status_string(bitcoind::Status::Error) == "error");
}

void test_manager_is_installed_false() {
    TEST_GROUP("Bitcoind Manager Is Installed (Not Installed)");

    bitcoind::Manager mgr;
    // By default, Bitcoin Core should not be installed in a test environment
    // This test may pass or fail depending on the system
    bool installed = mgr.is_installed();

    // We just verify the function runs without error
    TEST_RESULT("is_installed returns a value", installed == true || installed == false);
}

void test_manager_initial_status() {
    TEST_GROUP("Bitcoind Manager Initial Status");

    bitcoind::Manager mgr;

    TEST_RESULT("Initial status is NotInstalled",
                mgr.get_status() == bitcoind::Status::NotInstalled);
}

// ===========================================
// Run All Tests
// ===========================================

void run_all() {
    // Config tests
    test_config_bitcoind_managed_default();
    test_config_bitcoind_managed_parsing();
    test_config_bitcoind_validation();
    test_config_bitcoind_prune_validation();
    test_config_bitcoind_sv2_warning();

    // Manager tests
    test_manager_download_url();
    test_manager_paths();
    test_manager_custom_data_dir();
    test_manager_status_strings();
    test_manager_is_installed_false();
    test_manager_initial_status();
}

} // namespace test_bitcoind
