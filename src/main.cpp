#include "stratum.hpp"
#include "miner.hpp"
#include "config.hpp"
#include "webserver.hpp"

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
std::chrono::steady_clock::time_point g_start_time;

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

void setup_web_routes(web::WebServer& server) {
    // API: Get current stats
    server.get("/api/stats", [](const web::Request&, web::Response& res) {
        if (!g_miner) {
            res.json("{\"error\":\"Miner not running\"}");
            return;
        }

        const auto& stats = g_miner->get_stats();
        double uptime = std::chrono::duration<double>(
            std::chrono::steady_clock::now() - g_start_time).count();

        std::ostringstream oss;
        oss << "{"
            << "\"hashrate\":" << g_miner->get_hashrate() << ","
            << "\"total_hashes\":" << stats.hashes.load() << ","
            << "\"shares\":" << stats.shares_found.load() << ","
            << "\"threads\":" << g_config.threads << ","
            << "\"uptime\":" << uptime << ","
            << "\"connected\":" << (g_stratum && g_stratum->is_connected() ? "true" : "false")
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
        // Simple JSON parsing for settings
        // In production, use a proper JSON library
        res.json("{\"status\":\"ok\",\"message\":\"Settings received. Restart miner to apply.\"}");
        std::cout << "[Web] Settings update received: " << req.body << std::endl;
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

    // Start miner
    miner_instance.start();

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
    miner_instance.stop();
    client.stop();
    webserver.stop();

    g_miner = nullptr;
    g_stratum = nullptr;
    g_webserver = nullptr;

    if (stratum_thread.joinable()) {
        stratum_thread.join();
    }

    std::cout << "[Main] Goodbye!" << std::endl;
    return 0;
}
