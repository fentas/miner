#pragma once

#include <string>
#include <functional>
#include <atomic>
#include <thread>
#include <mutex>
#include <chrono>
#include <fstream>
#include <sstream>
#include <cstdlib>
#include <sys/stat.h>
#include <sys/types.h>
#include <signal.h>
#include <unistd.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <sys/wait.h>
#endif

namespace bitcoind {

// Bitcoin Core version to download
constexpr const char* BITCOIN_CORE_VERSION = "28.0";

// Download URLs by platform
inline std::string get_download_url() {
#if defined(__linux__)
    #if defined(__aarch64__)
        return "https://bitcoincore.org/bin/bitcoin-core-" + std::string(BITCOIN_CORE_VERSION) +
               "/bitcoin-" + std::string(BITCOIN_CORE_VERSION) + "-aarch64-linux-gnu.tar.gz";
    #else
        return "https://bitcoincore.org/bin/bitcoin-core-" + std::string(BITCOIN_CORE_VERSION) +
               "/bitcoin-" + std::string(BITCOIN_CORE_VERSION) + "-x86_64-linux-gnu.tar.gz";
    #endif
#elif defined(__APPLE__)
    #if defined(__aarch64__)
        return "https://bitcoincore.org/bin/bitcoin-core-" + std::string(BITCOIN_CORE_VERSION) +
               "/bitcoin-" + std::string(BITCOIN_CORE_VERSION) + "-arm64-apple-darwin.tar.gz";
    #else
        return "https://bitcoincore.org/bin/bitcoin-core-" + std::string(BITCOIN_CORE_VERSION) +
               "/bitcoin-" + std::string(BITCOIN_CORE_VERSION) + "-x86_64-apple-darwin.tar.gz";
    #endif
#elif defined(_WIN32)
    return "https://bitcoincore.org/bin/bitcoin-core-" + std::string(BITCOIN_CORE_VERSION) +
           "/bitcoin-" + std::string(BITCOIN_CORE_VERSION) + "-win64.zip";
#else
    return "";
#endif
}

// Status of Bitcoin Core
enum class Status {
    NotInstalled,
    Stopped,
    Starting,
    Syncing,
    Ready,
    Stopping,
    Error
};

inline std::string status_string(Status s) {
    switch (s) {
        case Status::NotInstalled: return "not_installed";
        case Status::Stopped: return "stopped";
        case Status::Starting: return "starting";
        case Status::Syncing: return "syncing";
        case Status::Ready: return "ready";
        case Status::Stopping: return "stopping";
        case Status::Error: return "error";
    }
    return "unknown";
}

// Sync progress info
struct SyncInfo {
    double progress = 0.0;          // 0.0 - 1.0
    int64_t headers = 0;
    int64_t blocks = 0;
    int64_t best_block_time = 0;
    bool initial_block_download = true;
};

// Block template for solo mining
struct BlockTemplate {
    bool valid = false;
    std::string version;            // Block version (hex, 4 bytes)
    std::string prev_block_hash;    // Previous block hash (hex, 32 bytes LE)
    std::string bits;               // Compact target (hex, 4 bytes)
    std::string target;             // Full target (hex, 32 bytes)
    uint32_t curtime = 0;           // Current time
    uint64_t height = 0;            // Block height
    uint64_t coinbase_value = 0;    // Block reward + fees (satoshis)
    std::vector<std::string> transactions;  // Serialized transactions (hex)
    std::vector<std::string> tx_hashes;     // Transaction hashes for merkle tree
    std::string coinbase_aux;       // Coinbase auxiliary data
    std::string error;              // Error message if invalid
};

// Block submission result
struct SubmitResult {
    bool accepted = false;
    std::string message;
    std::string block_hash;
};

// RPC response
struct RpcResponse {
    bool success = false;
    std::string result;
    std::string error;
};

// Callbacks
using StatusCallback = std::function<void(Status, const std::string&)>;
using ProgressCallback = std::function<void(double, const std::string&)>;

class Manager {
public:
    Manager() = default;
    ~Manager() { stop(); }

    // Configuration
    void set_data_dir(const std::string& dir) { data_dir_ = dir; }
    void set_rpc_credentials(const std::string& user, const std::string& pass) {
        rpc_user_ = user;
        rpc_password_ = pass;
    }
    void set_rpc_port(uint16_t port) { rpc_port_ = port; }
    void set_prune(unsigned prune_mb) { prune_mb_ = prune_mb; }
    void set_status_callback(StatusCallback cb) { status_callback_ = cb; }
    void set_progress_callback(ProgressCallback cb) { progress_callback_ = cb; }

    // Get paths
    std::string get_data_dir() const {
        if (!data_dir_.empty()) return data_dir_;

        const char* home = std::getenv("HOME");
        if (!home) home = std::getenv("USERPROFILE");
        if (!home) return "./bitcoin";

#ifdef _WIN32
        return std::string(home) + "\\AppData\\Roaming\\Bitcoin";
#elif defined(__APPLE__)
        return std::string(home) + "/Library/Application Support/Bitcoin";
#else
        return std::string(home) + "/.bitcoin";
#endif
    }

    std::string get_install_dir() const {
        const char* home = std::getenv("HOME");
        if (!home) home = std::getenv("USERPROFILE");
        if (!home) return "./bitcoin-core";

#ifdef _WIN32
        return std::string(home) + "\\AppData\\Local\\BitcoinCore";
#else
        return std::string(home) + "/.local/share/bitcoin-core";
#endif
    }

    std::string get_bitcoind_path() const {
        return get_install_dir() + "/bin/bitcoind";
    }

    std::string get_bitcoin_cli_path() const {
        return get_install_dir() + "/bin/bitcoin-cli";
    }

    // Check if installed
    bool is_installed() const {
        struct stat st;
        return stat(get_bitcoind_path().c_str(), &st) == 0;
    }

    // Download and install Bitcoin Core
    bool download_and_install(ProgressCallback progress = nullptr) {
        set_status(Status::Starting, "Downloading Bitcoin Core...");

        std::string url = get_download_url();
        if (url.empty()) {
            set_status(Status::Error, "Unsupported platform");
            return false;
        }

        std::string install_dir = get_install_dir();
        std::string tmp_dir = install_dir + "/tmp";

        // Create directories
        mkdir_p(install_dir);
        mkdir_p(tmp_dir);

        // Download file
        std::string archive = tmp_dir + "/bitcoin-core.tar.gz";
#ifdef _WIN32
        archive = tmp_dir + "/bitcoin-core.zip";
#endif

        if (progress) progress(0.1, "Downloading...");

        // Use curl or wget to download
        std::string cmd = "curl -L --progress-bar -o \"" + archive + "\" \"" + url + "\" 2>&1";
        int ret = std::system(cmd.c_str());

        if (ret != 0) {
            // Try wget
            cmd = "wget --progress=bar -O \"" + archive + "\" \"" + url + "\" 2>&1";
            ret = std::system(cmd.c_str());
        }

        if (ret != 0) {
            set_status(Status::Error, "Failed to download Bitcoin Core");
            return false;
        }

        if (progress) progress(0.6, "Extracting...");

        // Extract
#ifdef _WIN32
        cmd = "powershell -Command \"Expand-Archive -Path '" + archive + "' -DestinationPath '" + tmp_dir + "' -Force\"";
#else
        cmd = "tar -xzf \"" + archive + "\" -C \"" + tmp_dir + "\"";
#endif
        ret = std::system(cmd.c_str());

        if (ret != 0) {
            set_status(Status::Error, "Failed to extract Bitcoin Core");
            return false;
        }

        if (progress) progress(0.8, "Installing...");

        // Move binaries to install location
        std::string extracted_dir = tmp_dir + "/bitcoin-" + BITCOIN_CORE_VERSION;

#ifdef _WIN32
        cmd = "xcopy /E /Y \"" + extracted_dir + "\\*\" \"" + install_dir + "\\\"";
#else
        cmd = "cp -r \"" + extracted_dir + "/\"* \"" + install_dir + "/\"";
#endif
        ret = std::system(cmd.c_str());

        // Cleanup
        std::remove(archive.c_str());

        if (ret != 0) {
            set_status(Status::Error, "Failed to install Bitcoin Core");
            return false;
        }

        if (progress) progress(1.0, "Installed successfully");
        set_status(Status::Stopped, "Bitcoin Core installed");

        return true;
    }

    // Generate bitcoin.conf
    bool generate_config() {
        std::string data_dir = get_data_dir();
        mkdir_p(data_dir);

        std::string conf_path = data_dir + "/bitcoin.conf";

        // Check if config already exists
        struct stat st;
        if (stat(conf_path.c_str(), &st) == 0) {
            return true; // Already exists
        }

        // Generate random credentials if not set
        if (rpc_user_.empty()) {
            rpc_user_ = "btcminer";
        }
        if (rpc_password_.empty()) {
            rpc_password_ = generate_random_string(32);
        }

        std::ofstream conf(conf_path);
        if (!conf.is_open()) {
            set_status(Status::Error, "Failed to create bitcoin.conf");
            return false;
        }

        conf << "# Bitcoin Core configuration\n"
             << "# Generated by btcminer\n\n"
             << "# RPC settings\n"
             << "server=1\n"
             << "rpcuser=" << rpc_user_ << "\n"
             << "rpcpassword=" << rpc_password_ << "\n"
             << "rpcport=" << rpc_port_ << "\n"
             << "rpcbind=127.0.0.1\n"
             << "rpcallowip=127.0.0.1\n\n";

        if (prune_mb_ > 0) {
            conf << "# Pruning (saves disk space)\n"
                 << "prune=" << prune_mb_ << "\n\n";
        }

        conf << "# Performance settings\n"
             << "dbcache=450\n"
             << "maxmempool=300\n\n"
             << "# Network settings\n"
             << "listen=0\n"  // Don't accept incoming connections
             << "maxconnections=20\n";

        conf.close();
        return true;
    }

    // Start bitcoind
    bool start() {
        if (!is_installed()) {
            set_status(Status::Error, "Bitcoin Core not installed");
            return false;
        }

        if (status_ == Status::Ready || status_ == Status::Syncing || status_ == Status::Starting) {
            return true; // Already running
        }

        // Generate config if needed
        if (!generate_config()) {
            return false;
        }

        set_status(Status::Starting, "Starting Bitcoin Core...");

        std::string cmd = get_bitcoind_path();

        if (!data_dir_.empty()) {
            cmd += " -datadir=\"" + data_dir_ + "\"";
        }

        cmd += " -daemon";

#ifdef _WIN32
        // Windows: start detached process
        STARTUPINFO si = {0};
        PROCESS_INFORMATION pi = {0};
        si.cb = sizeof(si);

        if (!CreateProcess(NULL, (LPSTR)cmd.c_str(), NULL, NULL, FALSE,
                          CREATE_NO_WINDOW | DETACHED_PROCESS, NULL, NULL, &si, &pi)) {
            set_status(Status::Error, "Failed to start bitcoind");
            return false;
        }
        CloseHandle(pi.hProcess);
        CloseHandle(pi.hThread);
#else
        // Unix: fork and exec
        pid_t pid = fork();
        if (pid == 0) {
            // Child process
            setsid();

            std::string bitcoind = get_bitcoind_path();
            std::vector<char*> args;
            args.push_back(const_cast<char*>(bitcoind.c_str()));

            std::string datadir_arg;
            if (!data_dir_.empty()) {
                datadir_arg = "-datadir=" + data_dir_;
                args.push_back(const_cast<char*>(datadir_arg.c_str()));
            }

            args.push_back(nullptr);
            execv(bitcoind.c_str(), args.data());
            _exit(1);
        } else if (pid < 0) {
            set_status(Status::Error, "Failed to fork bitcoind process");
            return false;
        }
#endif

        // Start monitoring thread
        start_monitor();

        return true;
    }

    // Stop bitcoind
    bool stop() {
        if (status_ == Status::Stopped || status_ == Status::NotInstalled) {
            return true;
        }

        stop_monitor();
        set_status(Status::Stopping, "Stopping Bitcoin Core...");

        // Send stop command via RPC
        auto resp = rpc_call("stop");

        if (!resp.success) {
            // Try to kill process directly
#ifdef _WIN32
            (void)std::system("taskkill /F /IM bitcoind.exe 2>NUL");
#else
            (void)std::system("pkill -f bitcoind 2>/dev/null");
#endif
        }

        // Wait for process to exit
        for (int i = 0; i < 30; i++) {
            if (!is_running()) {
                set_status(Status::Stopped, "Bitcoin Core stopped");
                return true;
            }
            std::this_thread::sleep_for(std::chrono::seconds(1));
        }

        set_status(Status::Error, "Failed to stop Bitcoin Core");
        return false;
    }

    // Check if bitcoind is running
    bool is_running() const {
        auto resp = const_cast<Manager*>(this)->rpc_call("getblockchaininfo");
        return resp.success;
    }

    // Get sync status
    SyncInfo get_sync_info() {
        SyncInfo info;

        auto resp = rpc_call("getblockchaininfo");
        if (!resp.success) {
            return info;
        }

        // Parse JSON response (simple parsing without external library)
        auto get_value = [&resp](const std::string& key) -> std::string {
            size_t pos = resp.result.find("\"" + key + "\"");
            if (pos == std::string::npos) return "";
            pos = resp.result.find(":", pos);
            if (pos == std::string::npos) return "";
            pos++;
            while (pos < resp.result.size() && (resp.result[pos] == ' ' || resp.result[pos] == '"')) pos++;
            size_t end = pos;
            while (end < resp.result.size() && resp.result[end] != ',' &&
                   resp.result[end] != '}' && resp.result[end] != '"') end++;
            return resp.result.substr(pos, end - pos);
        };

        try {
            std::string prog = get_value("verificationprogress");
            if (!prog.empty()) info.progress = std::stod(prog);

            std::string headers = get_value("headers");
            if (!headers.empty()) info.headers = std::stoll(headers);

            std::string blocks = get_value("blocks");
            if (!blocks.empty()) info.blocks = std::stoll(blocks);

            std::string ibd = get_value("initialblockdownload");
            info.initial_block_download = (ibd == "true");
        } catch (...) {}

        return info;
    }

    // Get current status
    Status get_status() const { return status_; }
    std::string get_status_message() const { return status_message_; }

    // ========================================
    // Solo Mining Methods
    // ========================================

    // Get block template for mining (getblocktemplate RPC)
    BlockTemplate get_block_template(const std::string& payout_address) {
        BlockTemplate tmpl;

        // Check if node is synced
        auto sync_info = get_sync_info();
        if (sync_info.initial_block_download) {
            tmpl.error = "Node is still syncing (IBD). Wait for sync to complete.";
            return tmpl;
        }

        // Call getblocktemplate with segwit rules
        auto resp = rpc_call("getblocktemplate", "[{\"rules\": [\"segwit\"]}]");
        if (!resp.success) {
            tmpl.error = "getblocktemplate failed: " + resp.error;
            return tmpl;
        }

        // Parse JSON response (simple parsing)
        auto get_string = [&resp](const std::string& key) -> std::string {
            size_t pos = resp.result.find("\"" + key + "\"");
            if (pos == std::string::npos) return "";
            pos = resp.result.find(":", pos);
            if (pos == std::string::npos) return "";
            pos++;
            while (pos < resp.result.size() && (resp.result[pos] == ' ' || resp.result[pos] == '"')) pos++;
            size_t end = pos;
            bool in_quotes = (pos > 0 && resp.result[pos - 1] == '"');
            if (in_quotes) {
                end = resp.result.find("\"", pos);
            } else {
                while (end < resp.result.size() && resp.result[end] != ',' &&
                       resp.result[end] != '}' && resp.result[end] != '\n') end++;
            }
            return resp.result.substr(pos, end - pos);
        };

        auto get_number = [&resp](const std::string& key) -> uint64_t {
            size_t pos = resp.result.find("\"" + key + "\"");
            if (pos == std::string::npos) return 0;
            pos = resp.result.find(":", pos);
            if (pos == std::string::npos) return 0;
            pos++;
            while (pos < resp.result.size() && (resp.result[pos] == ' ')) pos++;
            try {
                return std::stoull(resp.result.substr(pos));
            } catch (...) {
                return 0;
            }
        };

        try {
            tmpl.version = get_string("version");
            if (tmpl.version.empty()) {
                // Version might be a number, convert to hex
                uint32_t ver = static_cast<uint32_t>(get_number("version"));
                char buf[16];
                snprintf(buf, sizeof(buf), "%08x", ver);
                tmpl.version = buf;
            }

            tmpl.prev_block_hash = get_string("previousblockhash");
            tmpl.bits = get_string("bits");
            tmpl.target = get_string("target");
            tmpl.curtime = static_cast<uint32_t>(get_number("curtime"));
            tmpl.height = get_number("height");
            tmpl.coinbase_value = get_number("coinbasevalue");

            // Parse transactions array
            size_t tx_start = resp.result.find("\"transactions\"");
            if (tx_start != std::string::npos) {
                tx_start = resp.result.find("[", tx_start);
                if (tx_start != std::string::npos) {
                    size_t tx_end = resp.result.find("]", tx_start);
                    std::string tx_array = resp.result.substr(tx_start, tx_end - tx_start + 1);

                    // Extract data fields from transactions
                    size_t pos = 0;
                    while ((pos = tx_array.find("\"data\"", pos)) != std::string::npos) {
                        pos = tx_array.find(":", pos);
                        if (pos == std::string::npos) break;
                        pos = tx_array.find("\"", pos);
                        if (pos == std::string::npos) break;
                        pos++;
                        size_t end = tx_array.find("\"", pos);
                        if (end != std::string::npos) {
                            tmpl.transactions.push_back(tx_array.substr(pos, end - pos));
                        }
                        pos = end;
                    }

                    // Extract txid fields for merkle tree
                    pos = 0;
                    while ((pos = tx_array.find("\"txid\"", pos)) != std::string::npos) {
                        pos = tx_array.find(":", pos);
                        if (pos == std::string::npos) break;
                        pos = tx_array.find("\"", pos);
                        if (pos == std::string::npos) break;
                        pos++;
                        size_t end = tx_array.find("\"", pos);
                        if (end != std::string::npos) {
                            tmpl.tx_hashes.push_back(tx_array.substr(pos, end - pos));
                        }
                        pos = end;
                    }
                }
            }

            tmpl.valid = !tmpl.prev_block_hash.empty() && !tmpl.bits.empty();

        } catch (const std::exception& e) {
            tmpl.error = "Failed to parse block template: " + std::string(e.what());
            tmpl.valid = false;
        }

        return tmpl;
    }

    // Submit a mined block to the network (submitblock RPC)
    SubmitResult submit_block(const std::string& block_hex) {
        SubmitResult result;

        if (block_hex.empty()) {
            result.message = "Empty block data";
            return result;
        }

        // submitblock returns null on success, or an error string on failure
        auto resp = rpc_call("submitblock", "\"" + block_hex + "\"");

        if (resp.success) {
            // Check response - null or empty means success
            std::string trimmed = resp.result;
            // Trim whitespace
            size_t start = trimmed.find_first_not_of(" \t\n\r");
            if (start == std::string::npos) {
                trimmed = "";
            } else {
                size_t end = trimmed.find_last_not_of(" \t\n\r");
                trimmed = trimmed.substr(start, end - start + 1);
            }

            if (trimmed.empty() || trimmed == "null") {
                result.accepted = true;
                result.message = "Block accepted!";

                // Get the block hash
                auto hash_resp = rpc_call("getbestblockhash");
                if (hash_resp.success) {
                    result.block_hash = hash_resp.result;
                    // Trim newline
                    size_t nl = result.block_hash.find('\n');
                    if (nl != std::string::npos) {
                        result.block_hash = result.block_hash.substr(0, nl);
                    }
                }
            } else {
                result.message = "Block rejected: " + trimmed;
            }
        } else {
            result.message = "submitblock failed: " + resp.error;
        }

        return result;
    }

    // Get network info (for displaying in UI)
    std::string get_network_difficulty() {
        auto resp = rpc_call("getdifficulty");
        if (resp.success) {
            // Trim newline
            size_t nl = resp.result.find('\n');
            if (nl != std::string::npos) {
                return resp.result.substr(0, nl);
            }
            return resp.result;
        }
        return "unknown";
    }

    // Get best block hash
    std::string get_best_block_hash() {
        auto resp = rpc_call("getbestblockhash");
        if (resp.success) {
            size_t nl = resp.result.find('\n');
            if (nl != std::string::npos) {
                return resp.result.substr(0, nl);
            }
            return resp.result;
        }
        return "";
    }

    // RPC call
    RpcResponse rpc_call(const std::string& method, const std::string& params = "") {
        RpcResponse response;

        std::string data_dir = get_data_dir();
        std::string cli = get_bitcoin_cli_path();

        if (!data_dir_.empty()) {
            cli += " -datadir=\"" + data_dir_ + "\"";
        }

        cli += " -rpcuser=" + rpc_user_;
        cli += " -rpcpassword=" + rpc_password_;
        cli += " -rpcport=" + std::to_string(rpc_port_);
        cli += " " + method;

        // Add parameters if provided
        if (!params.empty()) {
            // For bitcoin-cli, parameters are passed directly (not as JSON array)
            // Handle both raw params and JSON-formatted params
            if (params[0] == '[' || params[0] == '{' || params[0] == '"') {
                // JSON params - need to escape for shell and use -stdin for complex objects
                cli += " '" + params + "'";
            } else {
                cli += " " + params;
            }
        }

        // Execute and capture output
        std::string cmd = cli + " 2>&1";
        FILE* pipe = popen(cmd.c_str(), "r");
        if (!pipe) {
            response.error = "Failed to execute bitcoin-cli";
            return response;
        }

        char buffer[4096];
        while (fgets(buffer, sizeof(buffer), pipe) != nullptr) {
            response.result += buffer;
        }

        int status = pclose(pipe);
        response.success = (status == 0);

        if (!response.success) {
            response.error = response.result;
            response.result.clear();
        }

        return response;
    }

private:
    std::string data_dir_;
    std::string rpc_user_ = "btcminer";
    std::string rpc_password_;
    uint16_t rpc_port_ = 8332;
    unsigned prune_mb_ = 10000;

    std::atomic<Status> status_{Status::NotInstalled};
    std::string status_message_;
    std::mutex status_mutex_;

    std::thread monitor_thread_;
    std::atomic<bool> monitor_running_{false};

    StatusCallback status_callback_;
    ProgressCallback progress_callback_;

    void set_status(Status s, const std::string& msg) {
        std::lock_guard<std::mutex> lock(status_mutex_);
        status_ = s;
        status_message_ = msg;
        if (status_callback_) {
            status_callback_(s, msg);
        }
    }

    void start_monitor() {
        if (monitor_running_) return;

        monitor_running_ = true;
        monitor_thread_ = std::thread([this]() {
            while (monitor_running_) {
                auto info = get_sync_info();

                if (info.progress >= 0.9999) {
                    set_status(Status::Ready, "Fully synced");
                } else if (info.progress > 0) {
                    char msg[128];
                    snprintf(msg, sizeof(msg), "Syncing: %.2f%% (%lld/%lld blocks)",
                             info.progress * 100, (long long)info.blocks, (long long)info.headers);
                    set_status(Status::Syncing, msg);

                    if (progress_callback_) {
                        progress_callback_(info.progress, msg);
                    }
                }

                std::this_thread::sleep_for(std::chrono::seconds(5));
            }
        });
    }

    void stop_monitor() {
        monitor_running_ = false;
        if (monitor_thread_.joinable()) {
            monitor_thread_.join();
        }
    }

    static void mkdir_p(const std::string& path) {
#ifdef _WIN32
        std::system(("mkdir \"" + path + "\" 2>NUL").c_str());
#else
        std::system(("mkdir -p \"" + path + "\" 2>/dev/null").c_str());
#endif
    }

    static std::string generate_random_string(size_t length) {
        static const char charset[] = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
        std::string result;
        result.reserve(length);

        std::srand(static_cast<unsigned>(std::time(nullptr)));
        for (size_t i = 0; i < length; i++) {
            result += charset[std::rand() % (sizeof(charset) - 1)];
        }
        return result;
    }
};

} // namespace bitcoind
