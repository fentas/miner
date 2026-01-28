#pragma once

#include <string>
#include <fstream>
#include <sstream>
#include <iostream>
#include <algorithm>
#include <map>

namespace config {

struct Config {
    std::string pool_host = "solo.ckpool.org";
    uint16_t pool_port = 3333;
    std::string payout_address;
    std::string password = "x";
    unsigned threads = 0;
    unsigned stats_interval = 10;
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
        }
    }

    return true;
}

inline void print(const Config& cfg) {
    std::cout << "[Config] Pool: " << cfg.pool_host << ":" << cfg.pool_port << "\n"
              << "[Config] Address: " << cfg.payout_address << "\n"
              << "[Config] Threads: " << (cfg.threads == 0 ? "auto" : std::to_string(cfg.threads)) << "\n"
              << "[Config] Stats interval: " << cfg.stats_interval << "s\n"
              << std::endl;
}

} // namespace config
