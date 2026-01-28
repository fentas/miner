#include "stratum.hpp"
#include "miner.hpp"
#include "config.hpp"
#include "webserver.hpp"
#include "idle_monitor.hpp"
#include "colors.hpp"

#include <iostream>
#include <fstream>
#include <string>
#include <thread>
#include <chrono>
#include <csignal>
#include <cstdlib>
#include <cerrno>
#include <cstring>
#include <iomanip>
#include <sstream>
#include <atomic>
#include <unistd.h>

namespace {

std::atomic<bool> g_shutdown{false};
std::atomic<bool> g_restart{false};
config::Config g_config;
std::string g_config_file;
web::WebServer* g_webserver = nullptr;
miner::Miner* g_miner = nullptr;
stratum::Client* g_stratum = nullptr;
idle::IdleMonitor* g_idle_monitor = nullptr;
std::chrono::steady_clock::time_point g_start_time;
std::atomic<bool> g_idle_mining_active{false};
int g_argc = 0;
char** g_argv = nullptr;

void signal_handler(int) {
    std::cout << "\n" << colors::log_prefix("Main") << " " << colors::yellow() << "Shutting down..." << colors::reset() << std::endl;
    g_shutdown = true;
}

void print_usage(const char* prog) {
    std::cout << "Usage: " << prog << " [options]\n"
              << "\nOptions:\n"
              << "  -c, --config <file>      Config file (default: btcminer.conf)\n"
              << "  -o, --pool <host:port>   Pool address (default: solo.ckpool.org:3333)\n"
              << "  -u, --user <address>     Bitcoin address for mining rewards\n"
              << "  -p, --pass <password>    Pool password (default: x)\n"
              << "  -t, --threads <n>        Number of mining threads (default: all cores)\n"
              << "  -w, --web <port>         Web interface port (default: 8080, 0 to disable)\n"
              << "      --no-color           Disable colored output\n"
              << "      --color              Enable colored output (default)\n"
              << "  -h, --help               Show this help message\n"
              << "\nConfig file format (btcminer.conf):\n"
              << "  Pool = solo.ckpool.org:3333\n"
              << "  PayoutAddress = bc1q...\n"
              << "  Password = x\n"
              << "  Threads = 0\n"
              << "  WebPort = 8080\n"
              << "  Colors = true\n"
              << "\nExample:\n"
              << "  " << prog << " -c btcminer.conf\n"
              << "  " << prog << " -o solo.ckpool.org:3333 -u bc1q... -t 4 -w 8080\n"
              << std::endl;
}

std::string format_hashrate(double hr) {
    std::ostringstream oss;
    oss << std::fixed << std::setprecision(2);

    if (hr >= 1e12) {
        oss << (hr / 1e12) << " TH/s";
    } else if (hr >= 1e9) {
        oss << (hr / 1e9) << " GH/s";
    } else if (hr >= 1e6) {
        oss << (hr / 1e6) << " MH/s";
    } else if (hr >= 1e3) {
        oss << (hr / 1e3) << " KH/s";
    } else {
        oss << hr << " H/s";
    }
    return oss.str();
}

// Format time duration as human-readable string
std::string format_duration(double seconds) {
    if (seconds < 60) {
        return std::to_string(static_cast<int>(seconds)) + "s";
    } else if (seconds < 3600) {
        int mins = static_cast<int>(seconds / 60);
        int secs = static_cast<int>(seconds) % 60;
        return std::to_string(mins) + "m " + std::to_string(secs) + "s";
    } else if (seconds < 86400) {
        int hours = static_cast<int>(seconds / 3600);
        int mins = (static_cast<int>(seconds) % 3600) / 60;
        return std::to_string(hours) + "h " + std::to_string(mins) + "m";
    } else {
        int days = static_cast<int>(seconds / 86400);
        int hours = (static_cast<int>(seconds) % 86400) / 3600;
        return std::to_string(days) + "d " + std::to_string(hours) + "h";
    }
}

// Calculate estimated time to find a share at pool difficulty
// Formula: difficulty * 2^32 / hashrate
double estimate_time_to_share(double hashrate, double difficulty) {
    if (hashrate <= 0 || difficulty <= 0) return 0;
    return (difficulty * 4294967296.0) / hashrate;  // 2^32 = 4294967296
}

// Simple file logger - appends timestamped messages to a log file
std::ofstream g_log_file;

void log_to_file(const std::string& message) {
    if (g_log_file.is_open()) {
        auto now = std::chrono::system_clock::now();
        auto time = std::chrono::system_clock::to_time_t(now);
        char timestamp[32];
        std::strftime(timestamp, sizeof(timestamp), "%Y-%m-%d %H:%M:%S", std::localtime(&time));
        g_log_file << "[" << timestamp << "] " << message << std::endl;
    }
}

std::string state_to_string(miner::MinerState state) {
    switch (state) {
        case miner::MinerState::Stopped: return "stopped";
        case miner::MinerState::Running: return "running";
        case miner::MinerState::Paused: return "paused";
        case miner::MinerState::Benchmarking: return "benchmarking";
        default: return "unknown";
    }
}

bool save_config(const std::string& filename, const config::Config& cfg) {
    std::ofstream file(filename);
    if (!file.is_open()) {
        return false;
    }

    file << "# BTC Miner Configuration\n"
         << "# Generated by web interface\n\n"
         << "# Pool connection\n"
         << "Pool = " << cfg.pool_host << ":" << cfg.pool_port << "\n\n"
         << "# Your Bitcoin address for payouts\n"
         << "PayoutAddress = " << cfg.payout_address << "\n\n"
         << "# Pool password\n"
         << "Password = " << cfg.password << "\n\n"
         << "# Number of mining threads (0 = auto-detect)\n"
         << "Threads = " << cfg.threads << "\n\n"
         << "# Stats reporting interval in seconds\n"
         << "StatsInterval = " << cfg.stats_interval << "\n\n"
         << "# Web interface port (0 to disable)\n"
         << "WebPort = " << cfg.web_port << "\n\n"
         << "# Web files directory\n"
         << "WebDir = " << cfg.web_dir << "\n\n"
         << "# Idle mining timeout (seconds, 0 = disabled)\n"
         << "IdleTimeout = " << cfg.idle_timeout << "\n\n"
         << "# Enable colored console output\n"
         << "Colors = " << (cfg.colors ? "true" : "false") << "\n\n"
         << "# Log file (empty = disabled)\n"
         << "LogFile = " << cfg.log_file << "\n\n"
         << "# System bell on share found\n"
         << "SystemBell = " << (cfg.system_bell ? "true" : "false") << "\n\n"
         << "# Reconnection settings\n"
         << "ReconnectDelay = " << cfg.reconnect_delay << "\n"
         << "MaxReconnectDelay = " << cfg.max_reconnect_delay << "\n";

    return true;
}

void setup_web_routes(web::WebServer& server) {
    // API: Get current stats
    server.get("/api/stats", [](const web::Request&, web::Response& res) {
        if (!g_miner) {
            res.json("{\"error\":\"Miner not initialized\"}");
            return;
        }

        const auto& stats = g_miner->get_stats();
        double uptime = std::chrono::duration<double>(
            std::chrono::steady_clock::now() - g_start_time).count();

        double hashrate = g_miner->is_benchmarking() ?
            g_miner->get_benchmark_hashrate() : g_miner->get_hashrate();

        std::ostringstream oss;
        oss << "{"
            << "\"hashrate\":" << std::fixed << std::setprecision(0) << hashrate << ","
            << "\"total_hashes\":" << stats.hashes.load() << ","
            << "\"benchmark_hashes\":" << stats.benchmark_hashes.load() << ","
            << "\"shares\":" << stats.shares_found.load() << ","
            << "\"shares_accepted\":" << (g_stratum ? g_stratum->get_shares_accepted() : 0) << ","
            << "\"shares_rejected\":" << (g_stratum ? g_stratum->get_shares_rejected() : 0) << ","
            << "\"threads\":" << g_config.threads << ","
            << "\"uptime\":" << std::fixed << std::setprecision(1) << uptime << ","
            << "\"state\":\"" << state_to_string(g_miner->get_state()) << "\","
            << "\"connected\":" << (g_stratum && g_stratum->is_connected() ? "true" : "false") << ","
            << "\"idle_mining_enabled\":" << (g_config.idle_mining_enabled ? "true" : "false") << ","
            << "\"idle_mining_active\":" << (g_idle_mining_active ? "true" : "false") << ","
            << "\"user_idle\":" << (g_idle_monitor && g_idle_monitor->is_idle() ? "true" : "false")
            << "}";
        res.json(oss.str());
    });

    // API: Get settings
    server.get("/api/settings", [](const web::Request&, web::Response& res) {
        std::ostringstream oss;
        oss << "{"
            << "\"pool\":\"" << g_config.pool_host << ":" << g_config.pool_port << "\","
            << "\"address\":\"" << g_config.payout_address << "\","
            << "\"password\":\"" << g_config.password << "\","
            << "\"threads\":" << g_config.threads << ","
            << "\"stats_interval\":" << g_config.stats_interval << ","
            << "\"web_port\":" << g_config.web_port
            << "}";
        res.json(oss.str());
    });

    // API: Update settings (would require restart)
    server.post("/api/settings", [](const web::Request& req, web::Response& res) {
        res.json("{\"status\":\"ok\",\"message\":\"Settings received. Restart miner to apply.\"}");
        std::cout << "[Web] Settings update received: " << req.body << std::endl;
    });

    // API: Save settings and restart miner
    server.post("/api/control/restart", [](const web::Request& req, web::Response& res) {
        // Parse settings from request body (simple JSON parsing)
        config::Config new_cfg = g_config;  // Start with current config

        // Parse pool
        size_t pos = req.body.find("\"pool\"");
        if (pos != std::string::npos) {
            pos = req.body.find(':', pos);
            size_t start = req.body.find('"', pos + 1);
            size_t end = req.body.find('"', start + 1);
            if (start != std::string::npos && end != std::string::npos) {
                std::string pool = req.body.substr(start + 1, end - start - 1);
                size_t colon = pool.rfind(':');
                if (colon != std::string::npos) {
                    new_cfg.pool_host = pool.substr(0, colon);
                    try {
                        new_cfg.pool_port = static_cast<uint16_t>(std::stoul(pool.substr(colon + 1)));
                    } catch (...) {}
                } else {
                    new_cfg.pool_host = pool;
                }
            }
        }

        // Parse address
        pos = req.body.find("\"address\"");
        if (pos != std::string::npos) {
            pos = req.body.find(':', pos);
            size_t start = req.body.find('"', pos + 1);
            size_t end = req.body.find('"', start + 1);
            if (start != std::string::npos && end != std::string::npos) {
                new_cfg.payout_address = req.body.substr(start + 1, end - start - 1);
            }
        }

        // Parse password
        pos = req.body.find("\"password\"");
        if (pos != std::string::npos) {
            pos = req.body.find(':', pos);
            size_t start = req.body.find('"', pos + 1);
            size_t end = req.body.find('"', start + 1);
            if (start != std::string::npos && end != std::string::npos) {
                new_cfg.password = req.body.substr(start + 1, end - start - 1);
            }
        }

        // Parse threads
        pos = req.body.find("\"threads\"");
        if (pos != std::string::npos) {
            pos = req.body.find(':', pos);
            if (pos != std::string::npos) {
                try {
                    new_cfg.threads = static_cast<unsigned>(std::stoul(req.body.substr(pos + 1)));
                } catch (...) {}
            }
        }

        // Parse stats_interval
        pos = req.body.find("\"stats_interval\"");
        if (pos != std::string::npos) {
            pos = req.body.find(':', pos);
            if (pos != std::string::npos) {
                try {
                    new_cfg.stats_interval = static_cast<unsigned>(std::stoul(req.body.substr(pos + 1)));
                } catch (...) {}
            }
        }

        // Save config to file
        if (!save_config(g_config_file, new_cfg)) {
            res.json("{\"success\":false,\"message\":\"Failed to save config file\"}");
            return;
        }

        std::cout << colors::log_prefix("Web") << " Config saved, initiating restart..." << std::endl;
        res.json("{\"success\":true,\"message\":\"Config saved, restarting...\"}");

        // Signal restart
        g_restart = true;
        g_shutdown = true;
    });

    // API: Control - Start mining
    server.post("/api/control/start", [](const web::Request&, web::Response& res) {
        if (g_miner) {
            g_miner->start();
            res.json("{\"success\":true,\"state\":\"running\",\"message\":\"Mining started\"}");
        } else {
            res.json("{\"success\":false,\"message\":\"Miner not initialized\"}");
        }
    });

    // API: Control - Stop mining
    server.post("/api/control/stop", [](const web::Request&, web::Response& res) {
        if (g_miner) {
            g_miner->stop();
            res.json("{\"success\":true,\"state\":\"stopped\",\"message\":\"Mining stopped\"}");
        } else {
            res.json("{\"success\":false,\"message\":\"Miner not initialized\"}");
        }
    });

    // API: Control - Pause mining
    server.post("/api/control/pause", [](const web::Request&, web::Response& res) {
        if (g_miner) {
            g_miner->pause();
            res.json("{\"success\":true,\"state\":\"paused\",\"message\":\"Mining paused\"}");
        } else {
            res.json("{\"success\":false,\"message\":\"Miner not initialized\"}");
        }
    });

    // API: Control - Resume mining
    server.post("/api/control/resume", [](const web::Request&, web::Response& res) {
        if (g_miner) {
            g_miner->resume();
            res.json("{\"success\":true,\"state\":\"running\",\"message\":\"Mining resumed\"}");
        } else {
            res.json("{\"success\":false,\"message\":\"Miner not initialized\"}");
        }
    });

    // API: Control - Start benchmark
    server.post("/api/control/benchmark", [](const web::Request& req, web::Response& res) {
        if (!g_miner) {
            res.json("{\"success\":false,\"message\":\"Miner not initialized\"}");
            return;
        }

        // Parse duration from query string or request body
        unsigned duration = 30;  // Default 30 seconds

        // Check query string first (e.g., ?duration=10)
        size_t qpos = req.path.find("duration=");
        if (qpos != std::string::npos) {
            try {
                duration = static_cast<unsigned>(std::stoul(req.path.substr(qpos + 9)));
            } catch (...) {}
        } else {
            // Try request body
            size_t pos = req.body.find("duration");
            if (pos != std::string::npos) {
                pos = req.body.find(':', pos);
                if (pos != std::string::npos) {
                    try {
                        duration = static_cast<unsigned>(std::stoul(req.body.substr(pos + 1)));
                    } catch (...) {}
                }
            }
        }

        g_miner->start_benchmark(duration);
        res.json("{\"success\":true,\"state\":\"benchmarking\",\"message\":\"Benchmark started\",\"duration\":" +
                 std::to_string(duration) + "}");
    });

    // API: Control - Stop benchmark
    server.post("/api/control/benchmark/stop", [](const web::Request&, web::Response& res) {
        if (g_miner) {
            g_miner->stop_benchmark();
            res.json("{\"success\":true,\"state\":\"running\",\"message\":\"Benchmark stopped\"}");
        } else {
            res.json("{\"success\":false,\"message\":\"Miner not initialized\"}");
        }
    });

    // API: Get benchmark results
    server.get("/api/benchmark", [](const web::Request&, web::Response& res) {
        if (!g_miner) {
            res.json("{\"error\":\"Miner not initialized\"}");
            return;
        }

        const auto& stats = g_miner->get_stats();
        double hashrate = g_miner->get_benchmark_hashrate();
        double duration = g_miner->get_benchmark_duration();

        std::ostringstream oss;
        oss << "{"
            << "\"hashrate\":" << std::fixed << std::setprecision(0) << hashrate << ","
            << "\"total_hashes\":" << stats.benchmark_hashes.load() << ","
            << "\"duration\":" << std::fixed << std::setprecision(2) << duration << ","
            << "\"state\":\"" << state_to_string(g_miner->get_state()) << "\""
            << "}";
        res.json(oss.str());
    });

    // API: Get idle mining status
    server.get("/api/idle", [](const web::Request&, web::Response& res) {
        std::ostringstream oss;
        oss << "{"
            << "\"enabled\":" << (g_config.idle_mining_enabled ? "true" : "false") << ","
            << "\"timeout\":" << g_config.idle_timeout << ","
            << "\"is_idle\":" << (g_idle_monitor && g_idle_monitor->is_idle() ? "true" : "false") << ","
            << "\"idle_time\":" << (g_idle_monitor ? g_idle_monitor->get_idle_time() : 0) << ","
            << "\"mining_due_to_idle\":" << (g_idle_mining_active ? "true" : "false") << ","
            << "\"environment\":\"" << (g_idle_monitor ? idle::IdleMonitor::env_to_string(g_idle_monitor->get_environment()) : "unknown") << "\""
            << "}";
        res.json(oss.str());
    });

    // API: Enable/disable idle mining
    server.post("/api/idle/enable", [](const web::Request& req, web::Response& res) {
        // Parse timeout from query or body
        unsigned timeout = g_config.idle_timeout > 0 ? g_config.idle_timeout : 900;  // Default 15 min

        size_t qpos = req.path.find("timeout=");
        if (qpos != std::string::npos) {
            try {
                timeout = static_cast<unsigned>(std::stoul(req.path.substr(qpos + 8)));
            } catch (...) {}
        }

        g_config.idle_timeout = timeout;
        g_config.idle_mining_enabled = true;

        if (g_idle_monitor) {
            g_idle_monitor->set_timeout(timeout);
            g_idle_monitor->start();
        }

        res.json("{\"success\":true,\"message\":\"Idle mining enabled\",\"timeout\":" +
                 std::to_string(timeout) + "}");
    });

    // API: Disable idle mining
    server.post("/api/idle/disable", [](const web::Request&, web::Response& res) {
        g_config.idle_mining_enabled = false;

        if (g_idle_monitor) {
            g_idle_monitor->stop();
        }

        // If we were mining due to idle, stop
        if (g_idle_mining_active && g_miner) {
            g_miner->stop();
            g_idle_mining_active = false;
        }

        res.json("{\"success\":true,\"message\":\"Idle mining disabled\"}");
    });

    // API: Update idle timeout
    server.post("/api/idle/timeout", [](const web::Request& req, web::Response& res) {
        unsigned timeout = 900;

        size_t qpos = req.path.find("timeout=");
        if (qpos != std::string::npos) {
            try {
                timeout = static_cast<unsigned>(std::stoul(req.path.substr(qpos + 8)));
            } catch (...) {}
        }

        g_config.idle_timeout = timeout;
        if (g_idle_monitor) {
            g_idle_monitor->set_timeout(timeout);
        }

        res.json("{\"success\":true,\"timeout\":" + std::to_string(timeout) + "}");
    });
}

} // anonymous namespace

int main(int argc, char* argv[]) {
    // Store for potential restart
    g_argc = argc;
    g_argv = argv;

    // Configuration
    config::Config cfg;
    std::string config_file = "btcminer.conf";
    bool config_loaded = false;

    // First pass: check for config file argument
    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];
        if ((arg == "-c" || arg == "--config") && i + 1 < argc) {
            config_file = argv[++i];
            break;
        }
    }

    // Store config file path globally
    g_config_file = config_file;

    // Try to load config file
    if (config::load(config_file, cfg)) {
        std::cout << "[Main] Loaded config from: " << config_file << std::endl;
        config_loaded = true;
    }

    // Second pass: command line args override config file
    for (int i = 1; i < argc; i++) {
        std::string arg = argv[i];

        if (arg == "-h" || arg == "--help") {
            print_usage(argv[0]);
            return 0;
        } else if ((arg == "-c" || arg == "--config") && i + 1 < argc) {
            i++;  // Already handled
        } else if ((arg == "-o" || arg == "--pool") && i + 1 < argc) {
            std::string pool = argv[++i];
            size_t colon = pool.rfind(':');
            if (colon != std::string::npos) {
                cfg.pool_host = pool.substr(0, colon);
                cfg.pool_port = static_cast<uint16_t>(std::stoul(pool.substr(colon + 1)));
            } else {
                cfg.pool_host = pool;
            }
        } else if ((arg == "-u" || arg == "--user") && i + 1 < argc) {
            cfg.payout_address = argv[++i];
        } else if ((arg == "-p" || arg == "--pass") && i + 1 < argc) {
            cfg.password = argv[++i];
        } else if ((arg == "-t" || arg == "--threads") && i + 1 < argc) {
            cfg.threads = static_cast<unsigned>(std::stoul(argv[++i]));
        } else if ((arg == "-w" || arg == "--web") && i + 1 < argc) {
            cfg.web_port = static_cast<uint16_t>(std::stoul(argv[++i]));
        } else if (arg == "--no-color" || arg == "--no-colors") {
            cfg.colors = false;
        } else if (arg == "--color" || arg == "--colors") {
            cfg.colors = true;
        } else {
            std::cerr << "Unknown option: " << arg << std::endl;
            print_usage(argv[0]);
            return 1;
        }
    }

    // Apply color settings
    colors::enabled = cfg.colors;

    // Store config globally for web API
    g_config = cfg;

    // Validate configuration
    if (cfg.payout_address.empty()) {
        if (!config_loaded) {
            std::cerr << "Error: No config file found and no Bitcoin address specified.\n"
                      << "Create btcminer.conf or use -u <address>\n" << std::endl;
        } else {
            std::cerr << "Error: PayoutAddress not set in config file" << std::endl;
        }
        print_usage(argv[0]);
        return 1;
    }

    // Setup signal handler
    std::signal(SIGINT, signal_handler);
    std::signal(SIGTERM, signal_handler);

    // Open log file if configured
    if (!cfg.log_file.empty()) {
        g_log_file.open(cfg.log_file, std::ios::app);
        if (g_log_file.is_open()) {
            std::cout << "[Main] Logging to: " << cfg.log_file << std::endl;
            log_to_file("=== Miner started ===");
        } else {
            std::cerr << "[Main] Warning: Could not open log file: " << cfg.log_file << std::endl;
        }
    }

    // Record start time
    g_start_time = std::chrono::steady_clock::now();

    // Start web server if enabled (before banner so we know if it succeeded)
    web::WebServer webserver(cfg.web_port);
    bool web_started = false;
    if (cfg.web_port > 0) {
        g_webserver = &webserver;
        setup_web_routes(webserver);
        webserver.set_static_dir(cfg.web_dir);
        web_started = webserver.start();
    }

    // Print banner
    std::cout << "\n"
              << colors::cyan() << "╔══════════════════════════════════════════════════════════╗\n"
              << "║" << colors::bright_yellow() << "            BTC Miner - CPU Solo Mining                   " << colors::cyan() << "║\n"
              << "╠══════════════════════════════════════════════════════════╣" << colors::reset() << "\n"
              << colors::cyan() << "║" << colors::reset() << "  Pool: " << colors::bright_green() << std::left << std::setw(49) << (cfg.pool_host + ":" + std::to_string(cfg.pool_port)) << colors::cyan() << "║" << colors::reset() << "\n"
              << colors::cyan() << "║" << colors::reset() << "  User: " << colors::bright_cyan() << std::left << std::setw(49) << cfg.payout_address.substr(0, 49) << colors::cyan() << "║" << colors::reset() << "\n"
              << colors::cyan() << "║" << colors::reset() << "  Threads: " << colors::value() << std::left << std::setw(46) << (cfg.threads == 0 ? "auto" : std::to_string(cfg.threads)) << colors::cyan() << "║" << colors::reset() << "\n";

    if (web_started) {
        std::ostringstream web_info;
        web_info << "http://localhost:" << cfg.web_port;
        std::cout << colors::cyan() << "║" << colors::reset() << "  Web UI: " << colors::bright_blue() << std::left << std::setw(47) << web_info.str() << colors::cyan() << "║" << colors::reset() << "\n";
    }

    std::cout << colors::cyan() << "╚══════════════════════════════════════════════════════════╝" << colors::reset() << "\n"
              << std::endl;

    if (cfg.web_port > 0 && !web_started) {
        std::cerr << colors::log_prefix("Web") << " " << colors::error() << "Failed to start server on port " << cfg.web_port
                  << " (port in use?)" << colors::reset() << std::endl;
    }

    // Create miner
    miner::Miner miner_instance(cfg.threads);
    g_miner = &miner_instance;

    // Create stratum client
    stratum::Client client(cfg.pool_host, cfg.pool_port, cfg.payout_address, cfg.password);
    g_stratum = &client;

    // Setup callbacks
    client.set_job_callback([&miner_instance, &client](const stratum::Job& job) {
        // Generate random extranonce2
        std::ostringstream en2;
        size_t en2_size = client.get_extranonce2_size();
        for (size_t i = 0; i < en2_size; i++) {
            en2 << std::hex << std::setw(2) << std::setfill('0') << (rand() & 0xff);
        }
        miner_instance.set_job(job, en2.str());
    });

    miner_instance.set_share_callback(
        [&client](const std::string& job_id, const std::string& extranonce2,
                  const std::string& ntime, uint32_t nonce) {
            // Convert nonce to hex (big-endian for submission)
            std::ostringstream nonce_hex;
            nonce_hex << std::hex << std::setfill('0')
                      << std::setw(2) << ((nonce >> 24) & 0xff)
                      << std::setw(2) << ((nonce >> 16) & 0xff)
                      << std::setw(2) << ((nonce >> 8) & 0xff)
                      << std::setw(2) << (nonce & 0xff);
            client.submit(job_id, extranonce2, ntime, nonce_hex.str());
        });

    // Connect to pool
    if (!client.connect()) {
        std::cerr << "Failed to connect to pool" << std::endl;
        webserver.stop();
        return 1;
    }

    // Setup idle monitor
    idle::IdleMonitor idle_monitor;
    g_idle_monitor = &idle_monitor;

    if (cfg.idle_mining_enabled && cfg.idle_timeout > 0) {
        idle_monitor.set_timeout(cfg.idle_timeout);
        idle_monitor.set_callback([&miner_instance](bool is_idle) {
            if (is_idle) {
                // User went idle - start mining if not already running
                if (!miner_instance.is_running()) {
                    std::cout << "[Idle] User idle - starting mining" << std::endl;
                    miner_instance.start();
                    g_idle_mining_active = true;
                }
            } else {
                // User became active - stop mining if we started it due to idle
                if (g_idle_mining_active) {
                    std::cout << "[Idle] User active - stopping idle mining" << std::endl;
                    miner_instance.stop();
                    g_idle_mining_active = false;
                }
            }
        });
        idle_monitor.start();
        std::cout << "[Idle] Idle mining enabled (timeout: " << cfg.idle_timeout << "s)" << std::endl;

        // Don't auto-start miner if idle mining is enabled - wait for idle
        std::cout << "[Main] Waiting for idle state before mining..." << std::endl;
    } else {
        // Start miner immediately
        miner_instance.start();
    }

    // Start stratum client in separate thread
    std::atomic<bool> stratum_running{true};
    std::thread stratum_thread([&client, &stratum_running]() {
        client.run();
        stratum_running = false;
    });

    // Main loop - print stats, handle reconnection, broadcast to WebSocket
    auto last_stats_time = std::chrono::steady_clock::now();
    uint64_t last_hashes = 0;
    unsigned reconnect_delay = cfg.reconnect_delay;

    while (!g_shutdown) {
        // Check for disconnection and reconnect
        if (!client.is_connected() && !g_shutdown) {
            // Wait for stratum thread to finish
            if (stratum_thread.joinable() && !stratum_running) {
                stratum_thread.join();
            }

            std::cout << colors::log_prefix("Stratum") << " " << colors::yellow()
                      << "Connection lost. Reconnecting in " << reconnect_delay << "s..."
                      << colors::reset() << std::endl;
            log_to_file("Connection lost. Reconnecting in " + std::to_string(reconnect_delay) + "s");

            // Wait before reconnecting
            for (unsigned i = 0; i < reconnect_delay && !g_shutdown; i++) {
                std::this_thread::sleep_for(std::chrono::seconds(1));
            }

            if (g_shutdown) break;

            // Attempt reconnection
            if (client.connect()) {
                std::cout << colors::log_prefix("Stratum") << " " << colors::bright_green()
                          << "Reconnected successfully!" << colors::reset() << std::endl;
                log_to_file("Reconnected to pool");
                reconnect_delay = cfg.reconnect_delay;  // Reset delay on success

                // Restart stratum thread
                stratum_running = true;
                stratum_thread = std::thread([&client, &stratum_running]() {
                    client.run();
                    stratum_running = false;
                });
            } else {
                // Exponential backoff with max limit
                reconnect_delay = std::min(reconnect_delay * 2, cfg.max_reconnect_delay);
                std::cout << colors::log_prefix("Stratum") << " " << colors::error()
                          << "Reconnection failed. Next attempt in " << reconnect_delay << "s"
                          << colors::reset() << std::endl;
            }
            continue;
        }

        std::this_thread::sleep_for(std::chrono::seconds(cfg.stats_interval));

        auto now = std::chrono::steady_clock::now();
        auto elapsed = std::chrono::duration<double>(now - last_stats_time).count();
        auto uptime = std::chrono::duration<double>(now - g_start_time).count();

        const auto& stats = miner_instance.get_stats();
        uint64_t current_hashes = stats.hashes.load();
        double hashrate = (current_hashes - last_hashes) / elapsed;
        double difficulty = client.get_difficulty();
        uint64_t block_height = client.get_block_height();
        double est_time = estimate_time_to_share(hashrate, difficulty);

        // Build stats output
        std::ostringstream stats_line;
        stats_line << colors::log_prefix("Stats") << " Hashrate: " << colors::bright_green() << format_hashrate(hashrate) << colors::reset()
                   << " | Shares: " << colors::bright_cyan() << stats.shares_found.load() << colors::reset();

        if (block_height > 0) {
            stats_line << " | Block: " << colors::value() << block_height << colors::reset();
        }

        if (est_time > 0) {
            stats_line << " | ETA: " << colors::yellow() << "~" << format_duration(est_time) << colors::reset();
        }

        std::cout << stats_line.str() << std::endl;

        // Log to file
        if (g_log_file.is_open()) {
            std::ostringstream log_line;
            log_line << "Hashrate: " << format_hashrate(hashrate)
                     << " | Shares: " << stats.shares_found.load()
                     << " | Total: " << (current_hashes / 1000000) << "M";
            if (block_height > 0) log_line << " | Block: " << block_height;
            log_to_file(log_line.str());
        }

        // Broadcast to WebSocket clients
        if (cfg.web_port > 0) {
            unsigned actual_threads = cfg.threads == 0 ?
                std::thread::hardware_concurrency() : cfg.threads;

            std::ostringstream json;
            json << "{"
                 << "\"hashrate\":" << std::fixed << std::setprecision(0) << hashrate << ","
                 << "\"total_hashes\":" << current_hashes << ","
                 << "\"shares\":" << stats.shares_found.load() << ","
                 << "\"shares_accepted\":" << client.get_shares_accepted() << ","
                 << "\"shares_rejected\":" << client.get_shares_rejected() << ","
                 << "\"threads\":" << actual_threads << ","
                 << "\"uptime\":" << std::fixed << std::setprecision(1) << uptime << ","
                 << "\"block_height\":" << block_height << ","
                 << "\"difficulty\":" << std::fixed << std::setprecision(6) << difficulty << ","
                 << "\"est_time_to_share\":" << std::fixed << std::setprecision(0) << est_time
                 << "}";
            webserver.broadcast(json.str());
        }

        last_stats_time = now;
        last_hashes = current_hashes;
    }

    // Cleanup
    std::cout << "[Main] Stopping..." << std::endl;
    log_to_file("=== Miner stopped ===");

    idle_monitor.stop();
    miner_instance.stop();
    client.stop();
    webserver.stop();

    g_miner = nullptr;
    g_stratum = nullptr;
    g_webserver = nullptr;
    g_idle_monitor = nullptr;

    if (stratum_thread.joinable()) {
        stratum_thread.join();
    }

    // Close log file
    if (g_log_file.is_open()) {
        g_log_file.close();
    }

    // Check if we should restart
    if (g_restart) {
        std::cout << colors::log_prefix("Main") << " " << colors::bright_green() << "Restarting..." << colors::reset() << std::endl;

        // Small delay to let web response complete
        std::this_thread::sleep_for(std::chrono::milliseconds(500));

        // Re-exec ourselves
        execv(g_argv[0], g_argv);

        // If exec fails, print error and exit
        std::cerr << colors::log_prefix("Main") << " " << colors::error() << "Failed to restart: " << strerror(errno) << colors::reset() << std::endl;
        return 1;
    }

    std::cout << "[Main] Goodbye!" << std::endl;
    return 0;
}
