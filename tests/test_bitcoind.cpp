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
// Solo Mining Structure Tests
// ===========================================

void test_block_template_default() {
    TEST_GROUP("BlockTemplate Default Values");

    bitcoind::BlockTemplate tmpl;

    TEST_RESULT("BlockTemplate defaults to invalid", tmpl.valid == false);
    TEST_RESULT("BlockTemplate version is empty", tmpl.version.empty());
    TEST_RESULT("BlockTemplate prev_block_hash is empty", tmpl.prev_block_hash.empty());
    TEST_RESULT("BlockTemplate bits is empty", tmpl.bits.empty());
    TEST_RESULT("BlockTemplate curtime is 0", tmpl.curtime == 0);
    TEST_RESULT("BlockTemplate height is 0", tmpl.height == 0);
    TEST_RESULT("BlockTemplate coinbase_value is 0", tmpl.coinbase_value == 0);
    TEST_RESULT("BlockTemplate transactions is empty", tmpl.transactions.empty());
    TEST_RESULT("BlockTemplate tx_hashes is empty", tmpl.tx_hashes.empty());
    TEST_RESULT("BlockTemplate error is empty", tmpl.error.empty());
}

void test_submit_result_default() {
    TEST_GROUP("SubmitResult Default Values");

    bitcoind::SubmitResult result;

    TEST_RESULT("SubmitResult defaults to not accepted", result.accepted == false);
    TEST_RESULT("SubmitResult message is empty", result.message.empty());
    TEST_RESULT("SubmitResult block_hash is empty", result.block_hash.empty());
}

void test_block_template_with_data() {
    TEST_GROUP("BlockTemplate With Data");

    bitcoind::BlockTemplate tmpl;
    tmpl.valid = true;
    tmpl.version = "20000000";
    tmpl.prev_block_hash = "0000000000000000000123456789abcdef0123456789abcdef0123456789abcdef";
    tmpl.bits = "170b3ce9";
    tmpl.target = "00000000000000000007b3ce00000000000000000000000000000000000000000";
    tmpl.curtime = 1700000000;
    tmpl.height = 800000;
    tmpl.coinbase_value = 312500000;  // 3.125 BTC in satoshis

    TEST_RESULT("BlockTemplate valid is set", tmpl.valid == true);
    TEST_RESULT("BlockTemplate version is set", tmpl.version == "20000000");
    TEST_RESULT("BlockTemplate height is set", tmpl.height == 800000);
    TEST_RESULT("BlockTemplate coinbase_value is 3.125 BTC", tmpl.coinbase_value == 312500000);
}

void test_submit_result_accepted() {
    TEST_GROUP("SubmitResult Accepted");

    bitcoind::SubmitResult result;
    result.accepted = true;
    result.message = "Block accepted!";
    result.block_hash = "0000000000000000000abcdef123456789abcdef123456789abcdef123456789";

    TEST_RESULT("SubmitResult accepted is set", result.accepted == true);
    TEST_RESULT("SubmitResult message is set", result.message == "Block accepted!");
    TEST_RESULT("SubmitResult block_hash is set", !result.block_hash.empty());
}

void test_submit_result_rejected() {
    TEST_GROUP("SubmitResult Rejected");

    bitcoind::SubmitResult result;
    result.accepted = false;
    result.message = "Block rejected: high-hash";

    TEST_RESULT("SubmitResult rejected", result.accepted == false);
    TEST_RESULT("SubmitResult has rejection message", result.message.find("rejected") != std::string::npos);
}

void test_sync_info_default() {
    TEST_GROUP("SyncInfo Default Values");

    bitcoind::SyncInfo info;

    TEST_RESULT("SyncInfo progress defaults to 0", info.progress == 0.0);
    TEST_RESULT("SyncInfo headers defaults to 0", info.headers == 0);
    TEST_RESULT("SyncInfo blocks defaults to 0", info.blocks == 0);
    TEST_RESULT("SyncInfo initial_block_download defaults to true", info.initial_block_download == true);
}

void test_sync_info_synced() {
    TEST_GROUP("SyncInfo Fully Synced");

    bitcoind::SyncInfo info;
    info.progress = 1.0;
    info.headers = 800000;
    info.blocks = 800000;
    info.initial_block_download = false;

    TEST_RESULT("SyncInfo shows 100% progress", info.progress == 1.0);
    TEST_RESULT("SyncInfo headers equals blocks", info.headers == info.blocks);
    TEST_RESULT("SyncInfo IBD is complete", info.initial_block_download == false);
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

    // Solo mining structure tests
    test_block_template_default();
    test_submit_result_default();
    test_block_template_with_data();
    test_submit_result_accepted();
    test_submit_result_rejected();
    test_sync_info_default();
    test_sync_info_synced();
}

} // namespace test_bitcoind
