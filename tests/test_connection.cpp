// Pool Connection Tests
// Tests TCP connections and protocol handshakes to CKPool and Braiins

#include "test_framework.hpp"
#include "../src/config.hpp"

#include <string>
#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <unistd.h>
#include <fcntl.h>
#include <poll.h>
#include <cerrno>
#include <cstring>

namespace test_connection {

// Connection timeout in milliseconds
constexpr int CONNECT_TIMEOUT_MS = 5000;
constexpr int READ_TIMEOUT_MS = 5000;

// Helper: resolve hostname to IP
std::string resolve_host(const std::string& hostname) {
    struct addrinfo hints{}, *res;
    hints.ai_family = AF_INET;
    hints.ai_socktype = SOCK_STREAM;

    if (getaddrinfo(hostname.c_str(), nullptr, &hints, &res) != 0) {
        return "";
    }

    char ip[INET_ADDRSTRLEN];
    struct sockaddr_in* addr = reinterpret_cast<struct sockaddr_in*>(res->ai_addr);
    inet_ntop(AF_INET, &(addr->sin_addr), ip, INET_ADDRSTRLEN);

    freeaddrinfo(res);
    return std::string(ip);
}

// Helper: non-blocking connect with timeout
int connect_with_timeout(const std::string& host, uint16_t port, int timeout_ms) {
    std::string ip = resolve_host(host);
    if (ip.empty()) return -1;

    int sock = socket(AF_INET, SOCK_STREAM, 0);
    if (sock < 0) return -1;

    // Set non-blocking
    int flags = fcntl(sock, F_GETFL, 0);
    fcntl(sock, F_SETFL, flags | O_NONBLOCK);

    struct sockaddr_in addr{};
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    inet_pton(AF_INET, ip.c_str(), &addr.sin_addr);

    int result = connect(sock, reinterpret_cast<struct sockaddr*>(&addr), sizeof(addr));

    if (result < 0 && errno == EINPROGRESS) {
        struct pollfd pfd{};
        pfd.fd = sock;
        pfd.events = POLLOUT;

        result = poll(&pfd, 1, timeout_ms);

        if (result > 0) {
            int error = 0;
            socklen_t len = sizeof(error);
            getsockopt(sock, SOL_SOCKET, SO_ERROR, &error, &len);

            if (error != 0) {
                close(sock);
                return -1;
            }
        } else {
            close(sock);
            return -1;
        }
    } else if (result < 0) {
        close(sock);
        return -1;
    }

    // Reset to blocking mode
    fcntl(sock, F_SETFL, flags);

    return sock;
}

// Helper: send data to socket
bool send_data(int sock, const std::string& data) {
    ssize_t sent = send(sock, data.c_str(), data.size(), 0);
    return sent == static_cast<ssize_t>(data.size());
}

// Helper: receive line from socket with timeout
std::string recv_line(int sock, int timeout_ms) {
    std::string line;
    char buf[1];

    struct pollfd pfd{};
    pfd.fd = sock;
    pfd.events = POLLIN;

    while (poll(&pfd, 1, timeout_ms) > 0) {
        if (recv(sock, buf, 1, 0) <= 0) break;
        if (buf[0] == '\n') break;
        line += buf[0];
    }

    return line;
}

// ===========================================
// CKPool Tests (Stratum V1)
// ===========================================

void test_ckpool_dns_resolution() {
    TEST_GROUP("CKPool DNS Resolution");

    std::string ip = resolve_host("solo.ckpool.org");
    bool resolved = !ip.empty();

    TEST_RESULT("solo.ckpool.org resolves to IP", resolved);

    if (resolved) {
        // Should be a valid IPv4
        bool valid_ip = ip.find('.') != std::string::npos;
        TEST_RESULT("Resolved IP is valid format", valid_ip);
    }
}

void test_ckpool_tcp_connection() {
    TEST_GROUP("CKPool TCP Connection");

    int sock = connect_with_timeout("solo.ckpool.org", 3333, CONNECT_TIMEOUT_MS);
    bool connected = sock >= 0;

    TEST_RESULT("TCP connection to solo.ckpool.org:3333", connected);

    if (connected) {
        close(sock);
    }
}

void test_ckpool_stratum_subscribe() {
    TEST_GROUP("CKPool Stratum V1 Subscribe");

    int sock = connect_with_timeout("solo.ckpool.org", 3333, CONNECT_TIMEOUT_MS);
    if (sock < 0) {
        TEST_RESULT("(skipped - no connection)", false);
        return;
    }

    // Send subscribe message
    std::string subscribe = R"({"id":1,"method":"mining.subscribe","params":["btcminer/1.0"]})" "\n";
    bool sent = send_data(sock, subscribe);
    TEST_RESULT("Send mining.subscribe", sent);

    // Receive response
    std::string response = recv_line(sock, READ_TIMEOUT_MS);

    bool has_result = response.find("\"result\"") != std::string::npos;
    bool has_extranonce = response.find("mining.notify") != std::string::npos ||
                          response.find("\"id\":1") != std::string::npos;

    TEST_RESULT("Received subscribe response", !response.empty());
    TEST_RESULT("Response contains result", has_result || has_extranonce);

    close(sock);
}

void test_ckpool_stratum_authorize() {
    TEST_GROUP("CKPool Stratum V1 Authorize");

    int sock = connect_with_timeout("solo.ckpool.org", 3333, CONNECT_TIMEOUT_MS);
    if (sock < 0) {
        TEST_RESULT("(skipped - no connection)", false);
        return;
    }

    // Subscribe first
    std::string subscribe = R"({"id":1,"method":"mining.subscribe","params":["btcminer/1.0"]})" "\n";
    send_data(sock, subscribe);
    recv_line(sock, READ_TIMEOUT_MS);

    // Send authorize with test address
    std::string authorize = R"({"id":2,"method":"mining.authorize","params":["bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh","x"]})" "\n";
    bool sent = send_data(sock, authorize);
    TEST_RESULT("Send mining.authorize", sent);

    // Read multiple responses (pool sends notify/difficulty alongside auth response)
    bool authorized = false;
    for (int i = 0; i < 10; i++) {
        std::string response = recv_line(sock, READ_TIMEOUT_MS);
        if (response.empty()) break;

        // Look for authorize response with id:2 and result:true
        if (response.find("\"id\":2") != std::string::npos ||
            response.find("\"id\": 2") != std::string::npos) {
            authorized = response.find("\"result\":true") != std::string::npos ||
                         response.find("\"result\": true") != std::string::npos;
            break;
        }
    }

    TEST_RESULT("Authorization accepted", authorized);

    close(sock);
}

void test_ckpool_receives_job() {
    TEST_GROUP("CKPool Receives Mining Job");

    int sock = connect_with_timeout("solo.ckpool.org", 3333, CONNECT_TIMEOUT_MS);
    if (sock < 0) {
        TEST_RESULT("(skipped - no connection)", false);
        return;
    }

    // Subscribe
    std::string subscribe = R"({"id":1,"method":"mining.subscribe","params":["btcminer/1.0"]})" "\n";
    send_data(sock, subscribe);
    recv_line(sock, READ_TIMEOUT_MS);

    // Authorize
    std::string authorize = R"({"id":2,"method":"mining.authorize","params":["bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh","x"]})" "\n";
    send_data(sock, authorize);
    recv_line(sock, READ_TIMEOUT_MS);

    // Wait for mining.notify (job)
    bool received_job = false;
    for (int i = 0; i < 5; i++) {
        std::string msg = recv_line(sock, READ_TIMEOUT_MS);
        if (msg.find("mining.notify") != std::string::npos) {
            received_job = true;
            break;
        }
    }

    TEST_RESULT("Received mining.notify job", received_job);

    close(sock);
}

// ===========================================
// Braiins Tests (Stratum V1 - CKPool stack)
// ===========================================

void test_braiins_dns_resolution() {
    TEST_GROUP("Braiins DNS Resolution");

    std::string ip = resolve_host("solo.stratum.braiins.com");
    bool resolved = !ip.empty();

    TEST_RESULT("solo.stratum.braiins.com resolves to IP", resolved);

    if (resolved) {
        bool valid_ip = ip.find('.') != std::string::npos;
        TEST_RESULT("Resolved IP is valid format", valid_ip);
    }
}

void test_braiins_tcp_connection() {
    TEST_GROUP("Braiins TCP Connection");

    int sock = connect_with_timeout("solo.stratum.braiins.com", 3333, CONNECT_TIMEOUT_MS);
    bool connected = sock >= 0;

    TEST_RESULT("TCP connection to solo.stratum.braiins.com:3333", connected);

    if (connected) {
        close(sock);
    }
}

void test_braiins_stratum_v1_subscribe() {
    TEST_GROUP("Braiins Stratum V1 Subscribe");

    int sock = connect_with_timeout("solo.stratum.braiins.com", 3333, CONNECT_TIMEOUT_MS);
    if (sock < 0) {
        TEST_RESULT("(skipped - no connection)", false);
        return;
    }

    // Braiins Solo uses Stratum V1 (CKPool software stack)
    // Send subscribe message
    std::string subscribe = R"({"id":1,"method":"mining.subscribe","params":["btcminer/1.0"]})" "\n";
    bool sent = send_data(sock, subscribe);
    TEST_RESULT("Send mining.subscribe", sent);

    // Receive response
    std::string response = recv_line(sock, READ_TIMEOUT_MS);

    bool has_result = response.find("\"result\"") != std::string::npos;
    bool has_extranonce = response.find("mining.notify") != std::string::npos ||
                          response.find("\"id\":1") != std::string::npos;

    TEST_RESULT("Received subscribe response", !response.empty());
    TEST_RESULT("Response contains result", has_result || has_extranonce);

    close(sock);
}

// ===========================================
// Ocean Tests (Stratum V1 + DATUM)
// ===========================================

void test_ocean_dns_resolution() {
    TEST_GROUP("Ocean DNS Resolution");

    std::string ip = resolve_host("mine.ocean.xyz");
    bool resolved = !ip.empty();

    TEST_RESULT("mine.ocean.xyz resolves to IP", resolved);

    if (resolved) {
        bool valid_ip = ip.find('.') != std::string::npos;
        TEST_RESULT("Resolved IP is valid format", valid_ip);
    }
}

void test_ocean_tcp_connection() {
    TEST_GROUP("Ocean TCP Connection");

    int sock = connect_with_timeout("mine.ocean.xyz", 3334, CONNECT_TIMEOUT_MS);
    bool connected = sock >= 0;

    TEST_RESULT("TCP connection to mine.ocean.xyz:3334", connected);

    if (connected) {
        close(sock);
    }
}

void test_ocean_stratum_v1_subscribe() {
    TEST_GROUP("Ocean Stratum V1 Subscribe");

    int sock = connect_with_timeout("mine.ocean.xyz", 3334, CONNECT_TIMEOUT_MS);
    if (sock < 0) {
        TEST_RESULT("(skipped - no connection)", false);
        return;
    }

    // Ocean uses Stratum V1 with DATUM support
    std::string subscribe = R"({"id":1,"method":"mining.subscribe","params":["btcminer/1.0"]})" "\n";
    bool sent = send_data(sock, subscribe);
    TEST_RESULT("Send mining.subscribe", sent);

    std::string response = recv_line(sock, READ_TIMEOUT_MS);

    bool has_result = response.find("\"result\"") != std::string::npos;
    bool has_extranonce = response.find("mining.notify") != std::string::npos ||
                          response.find("\"id\":1") != std::string::npos;

    TEST_RESULT("Received subscribe response", !response.empty());
    TEST_RESULT("Response contains result", has_result || has_extranonce);

    close(sock);
}

// ===========================================
// Pool Preset Validation Tests
// ===========================================

void test_pool_presets_correct() {
    TEST_GROUP("Pool Presets Configuration");

    const auto& presets = config::get_pool_presets();

    // CKPool preset
    const auto& ckpool = presets.at(config::SoloPool::CKPool);
    TEST_RESULT("CKPool host is solo.ckpool.org", ckpool.host == "solo.ckpool.org");
    TEST_RESULT("CKPool port is 3333", ckpool.port == 3333);
    TEST_RESULT("CKPool uses Stratum V1", ckpool.stratum_version == "v1");

    // Braiins preset
    const auto& braiins = presets.at(config::SoloPool::Braiins);
    TEST_RESULT("Braiins host is solo.stratum.braiins.com", braiins.host == "solo.stratum.braiins.com");
    TEST_RESULT("Braiins port is 3333", braiins.port == 3333);
    TEST_RESULT("Braiins uses Stratum V1", braiins.stratum_version == "v1");

    // Ocean preset
    const auto& ocean = presets.at(config::SoloPool::Ocean);
    TEST_RESULT("Ocean host is mine.ocean.xyz", ocean.host == "mine.ocean.xyz");
    TEST_RESULT("Ocean port is 3334", ocean.port == 3334);
    TEST_RESULT("Ocean uses Stratum V1", ocean.stratum_version == "v1");
}

void test_config_applies_preset() {
    TEST_GROUP("Config Applies Pool Preset");

    // Test CKPool preset application
    config::Config cfg;
    cfg.solo_pool = config::SoloPool::CKPool;

    // Manually apply preset (simulating config load)
    const auto& preset = config::get_pool_presets().at(cfg.solo_pool);
    cfg.pool_host = preset.host;
    cfg.pool_port = preset.port;
    cfg.stratum_version = preset.stratum_version;

    TEST_RESULT("CKPool preset sets correct host", cfg.pool_host == "solo.ckpool.org");
    TEST_RESULT("CKPool preset sets correct port", cfg.pool_port == 3333);
    TEST_RESULT("CKPool preset sets correct stratum", cfg.stratum_version == "v1");

    // Test Braiins preset application
    cfg.solo_pool = config::SoloPool::Braiins;
    const auto& braiins = config::get_pool_presets().at(cfg.solo_pool);
    cfg.pool_host = braiins.host;
    cfg.pool_port = braiins.port;
    cfg.stratum_version = braiins.stratum_version;

    TEST_RESULT("Braiins preset sets correct host", cfg.pool_host == "solo.stratum.braiins.com");
    TEST_RESULT("Braiins preset sets correct port", cfg.pool_port == 3333);
    TEST_RESULT("Braiins preset sets correct stratum", cfg.stratum_version == "v1");

    // Test Ocean preset application
    cfg.solo_pool = config::SoloPool::Ocean;
    const auto& ocean = config::get_pool_presets().at(cfg.solo_pool);
    cfg.pool_host = ocean.host;
    cfg.pool_port = ocean.port;
    cfg.stratum_version = ocean.stratum_version;

    TEST_RESULT("Ocean preset sets correct host", cfg.pool_host == "mine.ocean.xyz");
    TEST_RESULT("Ocean preset sets correct port", cfg.pool_port == 3334);
    TEST_RESULT("Ocean preset sets correct stratum", cfg.stratum_version == "v1");
}

void run_all() {
    // CKPool (Stratum V1) tests
    test_ckpool_dns_resolution();
    test_ckpool_tcp_connection();
    test_ckpool_stratum_subscribe();
    test_ckpool_stratum_authorize();
    test_ckpool_receives_job();

    // Braiins (Stratum V1) tests
    test_braiins_dns_resolution();
    test_braiins_tcp_connection();
    test_braiins_stratum_v1_subscribe();

    // Ocean (Stratum V1 + DATUM) tests
    test_ocean_dns_resolution();
    test_ocean_tcp_connection();
    test_ocean_stratum_v1_subscribe();

    // Preset validation tests
    test_pool_presets_correct();
    test_config_applies_preset();
}

} // namespace test_connection
