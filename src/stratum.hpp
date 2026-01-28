#pragma once

#include <string>
#include <vector>
#include <mutex>
#include <atomic>
#include <functional>
#include <cstdint>

namespace stratum {

struct Job {
    std::string job_id;
    std::string prevhash;       // 32 bytes hex (64 chars)
    std::string coinb1;         // Coinbase part 1
    std::string coinb2;         // Coinbase part 2
    std::vector<std::string> merkle_branch;
    std::string version;        // 4 bytes hex (8 chars)
    std::string nbits;          // 4 bytes hex (8 chars)
    std::string ntime;          // 4 bytes hex (8 chars)
    bool clean_jobs;

    // Derived data
    uint8_t header_template[80];  // Pre-built header (without nonce)
    uint8_t target[32];           // Target for comparison
};

class Client {
public:
    using JobCallback = std::function<void(const Job&)>;

    Client(const std::string& host, uint16_t port,
           const std::string& user, const std::string& password);
    ~Client();

    bool connect();
    void disconnect();
    bool is_connected() const { return connected_; }

    bool submit(const std::string& job_id, const std::string& extranonce2,
                const std::string& ntime, const std::string& nonce);

    void set_job_callback(JobCallback cb) { job_callback_ = cb; }

    // Getters for extranonce
    std::string get_extranonce1() const { return extranonce1_; }
    size_t get_extranonce2_size() const { return extranonce2_size_; }

    // Run the receive loop (blocking)
    void run();

    // Request stop
    void stop() { running_ = false; }

private:
    bool send_json(const std::string& json);
    std::string recv_line();
    bool subscribe();
    bool authorize();
    void process_message(const std::string& msg);
    void process_notify(const std::vector<std::string>& params);

    void build_header(Job& job, const std::string& extranonce2);
    void calculate_target(Job& job);
    std::vector<uint8_t> hex_to_bytes(const std::string& hex);
    std::string bytes_to_hex(const uint8_t* data, size_t len);

    std::string host_;
    uint16_t port_;
    std::string user_;
    std::string password_;

    int sock_ = -1;
    std::atomic<bool> connected_{false};
    std::atomic<bool> running_{false};

    std::string extranonce1_;
    size_t extranonce2_size_ = 4;

    JobCallback job_callback_;
    std::mutex send_mutex_;

    int msg_id_ = 1;
};

} // namespace stratum
