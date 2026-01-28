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

    // RPC call
    RpcResponse rpc_call(const std::string& method, const std::string& params = "[]") {
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
