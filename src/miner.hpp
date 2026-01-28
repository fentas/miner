#pragma once

#include "stratum.hpp"
#include "sha256.hpp"

#include <atomic>
#include <thread>
#include <vector>
#include <mutex>
#include <condition_variable>
#include <functional>

namespace miner {

struct Stats {
    std::atomic<uint64_t> hashes{0};
    std::atomic<uint64_t> shares_found{0};
    std::atomic<uint64_t> shares_accepted{0};
    std::atomic<uint64_t> shares_rejected{0};
    std::atomic<uint64_t> benchmark_hashes{0};  // For benchmark mode
};

enum class MinerState {
    Stopped,
    Running,
    Paused,
    Benchmarking
};

class Miner {
public:
    using ShareCallback = std::function<void(const std::string& job_id,
                                              const std::string& extranonce2,
                                              const std::string& ntime,
                                              uint32_t nonce)>;

    Miner(unsigned num_threads = 0);
    ~Miner();

    void start();
    void stop();
    void pause();
    void resume();

    // Benchmark mode - mine without submitting
    void start_benchmark(unsigned duration_seconds = 0);
    void stop_benchmark();
    double get_benchmark_hashrate() const;
    double get_benchmark_duration() const;

    void set_job(const stratum::Job& job, const std::string& extranonce2);
    void set_share_callback(ShareCallback cb) { share_callback_ = cb; }

    const Stats& get_stats() const { return stats_; }
    double get_hashrate() const;
    MinerState get_state() const { return state_.load(); }
    bool is_running() const { return state_ == MinerState::Running || state_ == MinerState::Benchmarking; }
    bool is_paused() const { return state_ == MinerState::Paused; }
    bool is_benchmarking() const { return state_ == MinerState::Benchmarking; }

private:
    void worker_thread(unsigned thread_id);
    bool check_hash(const uint8_t* hash) const;
    std::string nonce_to_hex(uint32_t nonce) const;

    unsigned num_threads_;
    std::vector<std::thread> threads_;
    std::atomic<bool> running_{false};
    std::atomic<bool> has_job_{false};
    std::atomic<bool> paused_{false};
    std::atomic<MinerState> state_{MinerState::Stopped};

    // Benchmark
    std::atomic<bool> benchmark_mode_{false};
    std::chrono::steady_clock::time_point benchmark_start_;
    unsigned benchmark_duration_{0};  // 0 = unlimited

    // Current job data (protected by mutex)
    std::mutex job_mutex_;
    std::condition_variable job_cv_;
    stratum::Job current_job_;
    std::string current_extranonce2_;
    sha256::Midstate midstate_;
    std::atomic<uint64_t> job_id_{0};  // Incremented on new job

    // Nonce distribution
    std::atomic<uint32_t> nonce_base_{0};
    static constexpr uint32_t NONCE_CHUNK = 0x100000;  // 1M nonces per chunk

    Stats stats_;
    ShareCallback share_callback_;

    // Hashrate calculation
    std::chrono::steady_clock::time_point start_time_;
};

} // namespace miner
