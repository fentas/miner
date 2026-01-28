#include "stratum.hpp"
#include "miner.hpp"
#include "config.hpp"
#include "webserver.hpp"
#include "idle_monitor.hpp"

#include <iostream>
#include <string>
#include <thread>
#include <chrono>
#include <csignal>
#include <cstdlib>
#include <iomanip>
#include <sstream>
#include <atomic>

namespace {

std::atomic<bool> g_shutdown{false};
config::Config g_config;
web::WebServer* g_webserver = nullptr;
miner::Miner* g_miner = nullptr;
stratum::Client* g_stratum = nullptr;
idle::IdleMonitor* g_idle_monitor = nullptr;
std::chrono::steady_clock::time_point g_start_time;
std::atomic<bool> g_idle_mining_active{false};

void signal_handler(int) {
    std::cout << "\n[Main] Shutting down..." << std::endl;
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
              << "  -h, --help               Show this help message\n"
              << "\nConfig file format (btcminer.conf):\n"
              << "  Pool = solo.ckpool.org:3333\n"
              << "  PayoutAddress = bc1q...\n"
              << "  Password = x\n"
              << "  Threads = 0\n"
              << "  WebPort = 8080\n"
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

std::string make_stats_json(double hashrate, uint64_t total_hashes, uint64_t shares,
                            unsigned threads, double uptime) {
    std::ostringstream oss;
    oss << "{"
        << "\"hashrate\":" << std::fixed << std::setprecision(0) << hashrate << ","
        << "\"total_hashes\":" << total_hashes << ","
        << "\"shares\":" << shares << ","
        << "\"threads\":" << threads << ","
        << "\"uptime\":" << std::fixed << std::setprecision(1) << uptime
        << "}";
    return oss.str();
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
        } else {
            std::cerr << "Unknown option: " << arg << std::endl;
            print_usage(argv[0]);
            return 1;
        }
    }

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

    // Print banner
    std::cout << "\n"
              << "╔══════════════════════════════════════════════════════════╗\n"
              << "║            BTC Miner - CPU Solo Mining                   ║\n"
              << "╠══════════════════════════════════════════════════════════╣\n"
              << "║  Pool: " << std::left << std::setw(49) << (cfg.pool_host + ":" + std::to_string(cfg.pool_port)) << "║\n"
              << "║  User: " << std::left << std::setw(49) << cfg.payout_address.substr(0, 49) << "║\n"
              << "║  Threads: " << std::left << std::setw(46) << (cfg.threads == 0 ? "auto" : std::to_string(cfg.threads)) << "║\n";

    if (cfg.web_port > 0) {
        std::ostringstream web_info;
        web_info << "http://localhost:" << cfg.web_port;
        std::cout << "║  Web UI: " << std::left << std::setw(47) << web_info.str() << "║\n";
    }

    std::cout << "╚══════════════════════════════════════════════════════════╝\n"
              << std::endl;

    // Setup signal handler
    std::signal(SIGINT, signal_handler);
    std::signal(SIGTERM, signal_handler);

    // Record start time
    g_start_time = std::chrono::steady_clock::now();

    // Start web server if enabled
    web::WebServer webserver(cfg.web_port);
    if (cfg.web_port > 0) {
        g_webserver = &webserver;
        setup_web_routes(webserver);
        webserver.set_static_dir(cfg.web_dir);

        if (webserver.start()) {
            std::cout << "[Web] Server started on port " << cfg.web_port << std::endl;
        } else {
            std::cerr << "[Web] Failed to start server on port " << cfg.web_port << std::endl;
        }
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
    std::thread stratum_thread([&client]() {
        client.run();
    });

    // Main loop - print stats and broadcast to WebSocket
    auto last_stats_time = std::chrono::steady_clock::now();
    uint64_t last_hashes = 0;

    while (!g_shutdown && client.is_connected()) {
        std::this_thread::sleep_for(std::chrono::seconds(cfg.stats_interval));

        auto now = std::chrono::steady_clock::now();
        auto elapsed = std::chrono::duration<double>(now - last_stats_time).count();
        auto uptime = std::chrono::duration<double>(now - g_start_time).count();

        const auto& stats = miner_instance.get_stats();
        uint64_t current_hashes = stats.hashes.load();
        double hashrate = (current_hashes - last_hashes) / elapsed;

        std::cout << "[Stats] Hashrate: " << format_hashrate(hashrate)
                  << " | Total: " << (current_hashes / 1000000) << "M hashes"
                  << " | Shares: " << stats.shares_found.load()
                  << std::endl;

        // Broadcast to WebSocket clients
        if (cfg.web_port > 0) {
            unsigned actual_threads = cfg.threads == 0 ?
                std::thread::hardware_concurrency() : cfg.threads;
            std::string json = make_stats_json(hashrate, current_hashes,
                                               stats.shares_found.load(),
                                               actual_threads, uptime);
            webserver.broadcast(json);
        }

        last_stats_time = now;
        last_hashes = current_hashes;
    }

    // Cleanup
    std::cout << "[Main] Stopping..." << std::endl;
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

    std::cout << "[Main] Goodbye!" << std::endl;
    return 0;
}
