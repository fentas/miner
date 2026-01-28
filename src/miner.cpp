#include "miner.hpp"
#include "colors.hpp"

#include <iostream>
#include <iomanip>
#include <sstream>
#include <cstring>
#include <random>

namespace miner {

Miner::Miner(unsigned num_threads)
    : num_threads_(num_threads ? num_threads : std::thread::hardware_concurrency()) {
    if (num_threads_ == 0) num_threads_ = 1;
    std::cout << colors::log_prefix("Miner") << " Using " << colors::value() << num_threads_ << colors::reset() << " threads" << std::endl;
}

Miner::~Miner() {
    stop();
}

void Miner::start() {
    if (running_) return;

    running_ = true;
    paused_ = false;
    benchmark_mode_ = false;
    state_ = MinerState::Running;
    start_time_ = std::chrono::steady_clock::now();

    threads_.reserve(num_threads_);
    for (unsigned i = 0; i < num_threads_; i++) {
        threads_.emplace_back(&Miner::worker_thread, this, i);
    }

    std::cout << "[Miner] Started " << num_threads_ << " mining threads" << std::endl;
}

void Miner::stop() {
    running_ = false;
    paused_ = false;
    has_job_ = false;
    benchmark_mode_ = false;
    state_ = MinerState::Stopped;
    job_cv_.notify_all();

    for (auto& t : threads_) {
        if (t.joinable()) {
            t.join();
        }
    }
    threads_.clear();
    std::cout << "[Miner] Stopped" << std::endl;
}

void Miner::pause() {
    if (!running_ || paused_) return;
    paused_ = true;
    state_ = MinerState::Paused;
    std::cout << "[Miner] Paused" << std::endl;
}

void Miner::resume() {
    if (!running_ || !paused_) return;
    paused_ = false;
    state_ = benchmark_mode_ ? MinerState::Benchmarking : MinerState::Running;
    job_cv_.notify_all();
    std::cout << "[Miner] Resumed" << std::endl;
}

void Miner::start_benchmark(unsigned duration_seconds) {
    // Create a dummy job for benchmarking
    stratum::Job dummy_job;
    dummy_job.job_id = "benchmark";
    std::memset(dummy_job.header_template, 0x42, 80);
    std::memset(dummy_job.target, 0xFF, 32);  // Easy target - accept all
    dummy_job.target[31] = 0x00;  // But require at least 1 leading zero byte
    dummy_job.target[30] = 0x00;
    dummy_job.ntime = "00000000";

    benchmark_mode_ = true;
    benchmark_duration_ = duration_seconds;
    stats_.benchmark_hashes = 0;
    benchmark_start_ = std::chrono::steady_clock::now();

    if (!running_) {
        running_ = true;
        paused_ = false;
        state_ = MinerState::Benchmarking;
        start_time_ = std::chrono::steady_clock::now();

        threads_.reserve(num_threads_);
        for (unsigned i = 0; i < num_threads_; i++) {
            threads_.emplace_back(&Miner::worker_thread, this, i);
        }
    } else {
        state_ = MinerState::Benchmarking;
    }

    // Set the dummy job
    {
        std::lock_guard<std::mutex> lock(job_mutex_);
        current_job_ = dummy_job;
        current_extranonce2_ = "00000000";
        sha256::compute_midstate(current_job_.header_template, midstate_);
        nonce_base_ = 0;
        job_id_++;
    }

    has_job_ = true;
    job_cv_.notify_all();

    std::cout << "[Miner] Benchmark started"
              << (duration_seconds > 0 ? " for " + std::to_string(duration_seconds) + "s" : "")
              << std::endl;
}

void Miner::stop_benchmark() {
    benchmark_mode_ = false;
    has_job_ = false;
    state_ = MinerState::Running;
    std::cout << "[Miner] Benchmark stopped" << std::endl;
}

double Miner::get_benchmark_hashrate() const {
    auto now = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration<double>(now - benchmark_start_).count();
    if (elapsed < 0.001) return 0.0;
    return static_cast<double>(stats_.benchmark_hashes.load()) / elapsed;
}

double Miner::get_benchmark_duration() const {
    auto now = std::chrono::steady_clock::now();
    return std::chrono::duration<double>(now - benchmark_start_).count();
}

void Miner::set_job(const stratum::Job& job, const std::string& extranonce2) {
    // Don't accept new jobs in benchmark mode
    if (benchmark_mode_) return;

    {
        std::lock_guard<std::mutex> lock(job_mutex_);
        current_job_ = job;
        current_extranonce2_ = extranonce2;

        // Compute midstate for the first 64 bytes of the header
        sha256::compute_midstate(current_job_.header_template, midstate_);

        // Random nonce start - avoids re-scanning same nonces after restart
        // and spreads nonce coverage across the full 2^32 space
        std::random_device rd;
        std::mt19937 gen(rd());
        std::uniform_int_distribution<uint32_t> dist(0, 0xFFFFFFFF);
        nonce_base_ = dist(gen);
        job_id_++;
    }

    has_job_ = true;
    job_cv_.notify_all();

    std::cout << "[Miner] New job set: " << job.job_id << std::endl;
}

double Miner::get_hashrate() const {
    auto now = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration<double>(now - start_time_).count();
    if (elapsed < 0.001) return 0.0;
    return static_cast<double>(stats_.hashes.load()) / elapsed;
}

void Miner::worker_thread(unsigned thread_id) {
    uint8_t hash[32];
    uint64_t local_hashes = 0;
    uint64_t last_job_id = 0;

    // Local copies for speed
    sha256::Midstate local_midstate;
    uint8_t local_target[32];
    std::string local_job_id;
    std::string local_extranonce2;
    std::string local_ntime;
    bool local_benchmark_mode = false;

    std::cout << "[Miner] Thread " << thread_id << " started" << std::endl;

    while (running_) {
        // Check for pause
        while (paused_ && running_) {
            std::this_thread::sleep_for(std::chrono::milliseconds(100));
        }

        if (!running_) break;

        // Wait for job
        {
            std::unique_lock<std::mutex> lock(job_mutex_);
            job_cv_.wait(lock, [this] { return (has_job_ && !paused_) || !running_; });

            if (!running_) break;

            // Copy job data locally
            local_midstate = midstate_;
            std::memcpy(local_target, current_job_.target, 32);
            local_job_id = current_job_.job_id;
            local_extranonce2 = current_extranonce2_;
            local_ntime = current_job_.ntime;
            last_job_id = job_id_;
            local_benchmark_mode = benchmark_mode_.load();
        }

        // Check benchmark duration
        if (local_benchmark_mode && benchmark_duration_ > 0) {
            auto elapsed = std::chrono::steady_clock::now() - benchmark_start_;
            if (std::chrono::duration<double>(elapsed).count() >= benchmark_duration_) {
                if (thread_id == 0) {
                    std::cout << "[Miner] Benchmark duration reached" << std::endl;
                    benchmark_mode_ = false;
                    has_job_ = false;
                }
                continue;
            }
        }

        // Get nonce range for this thread
        uint32_t nonce_start = nonce_base_.fetch_add(NONCE_CHUNK);
        uint32_t nonce_end = nonce_start + NONCE_CHUNK;

        // Mine!
        for (uint32_t nonce = nonce_start; nonce < nonce_end && running_ && !paused_; nonce++) {
            // Check if job changed
            if (job_id_ != last_job_id) break;

            sha256::sha256d_midstate(local_midstate, nonce, hash);
            local_hashes++;

            // Update stats periodically
            if ((local_hashes & 0xFFFF) == 0) {
                if (local_benchmark_mode) {
                    stats_.benchmark_hashes += 0x10000;
                } else {
                    stats_.hashes += 0x10000;
                }
                local_hashes = 0;
            }

            // Check if hash meets target (skip submission in benchmark mode)
            if (check_hash(hash)) {
                if (local_benchmark_mode) {
                    // In benchmark mode, just count but don't submit
                    continue;
                }

                stats_.shares_found++;

                // System bell for audio notification
                std::cout << '\a' << std::flush;

                // Count leading zero bytes in hash (indicator of difficulty)
                int leading_zeros = 0;
                for (int i = 31; i >= 0 && hash[i] == 0; i--) {
                    leading_zeros++;
                }

                // BIG celebration banner!
                std::cout << "\n"
                          << colors::bright_green() << "╔════════════════════════════════════════════════════════════╗\n"
                          << "║" << colors::bright_yellow() << "  ★★★  " << colors::bright_green()
                          << (leading_zeros >= 4 ? "POTENTIAL BLOCK FOUND!!!" : "SHARE FOUND!")
                          << (leading_zeros >= 4 ? "  ★★★" : "          ★★★")
                          << colors::bright_green() << "  ║\n"
                          << "╚════════════════════════════════════════════════════════════╝"
                          << colors::reset() << "\n";

                std::cout << colors::log_prefix("Miner") << " " << colors::bright_green() << "Share #"
                          << stats_.shares_found.load() << colors::reset()
                          << " | Nonce: " << colors::bright_yellow() << "0x"
                          << std::hex << std::setw(8) << std::setfill('0') << nonce
                          << std::dec << colors::reset()
                          << " | Leading zeros: " << colors::bright_cyan() << leading_zeros << colors::reset()
                          << std::endl;

                // Print hash (reversed for display - big endian)
                std::cout << colors::log_prefix("Miner") << " Hash: " << colors::muted();
                for (int i = 31; i >= 0; i--) {
                    if (hash[i] == 0 && i > 24) {
                        std::cout << colors::bright_green() << "00" << colors::muted();
                    } else {
                        std::cout << std::hex << std::setw(2) << std::setfill('0')
                                  << static_cast<int>(hash[i]);
                    }
                }
                std::cout << std::dec << colors::reset() << "\n" << std::endl;

                if (share_callback_) {
                    share_callback_(local_job_id, local_extranonce2,
                                   local_ntime, nonce);
                }
            }
        }

        // Add remaining hashes to stats
        if (local_benchmark_mode) {
            stats_.benchmark_hashes += local_hashes;
        } else {
            stats_.hashes += local_hashes;
        }
        local_hashes = 0;
    }

    std::cout << "[Miner] Thread " << thread_id << " stopped" << std::endl;
}

bool Miner::check_hash(const uint8_t* hash) const {
    // Compare hash against target (both in little-endian)
    // Hash must be less than target
    for (int i = 31; i >= 0; i--) {
        if (hash[i] < current_job_.target[i]) return true;
        if (hash[i] > current_job_.target[i]) return false;
    }
    return true;  // Equal to target is valid
}

std::string Miner::nonce_to_hex(uint32_t nonce) const {
    std::ostringstream oss;
    // Little-endian hex
    oss << std::hex << std::setfill('0')
        << std::setw(2) << (nonce & 0xff)
        << std::setw(2) << ((nonce >> 8) & 0xff)
        << std::setw(2) << ((nonce >> 16) & 0xff)
        << std::setw(2) << ((nonce >> 24) & 0xff);
    return oss.str();
}

} // namespace miner
