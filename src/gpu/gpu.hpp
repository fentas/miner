#pragma once

// GPU Mining Interface - Abstract base for CUDA and OpenCL implementations

#include <cstdint>
#include <string>
#include <vector>
#include <memory>
#include <functional>
#include <atomic>
#include <thread>
#include <mutex>

namespace gpu {

// GPU device information
struct DeviceInfo {
    int id;
    std::string name;
    std::string vendor;
    uint64_t memory_total;      // bytes
    uint64_t memory_free;       // bytes
    unsigned compute_units;     // SMs for CUDA, CUs for OpenCL
    unsigned max_threads;       // per compute unit
    bool available;
    std::string backend;        // "cuda" or "opencl"
};

// Mining result from GPU
struct MiningResult {
    uint32_t nonce;
    uint8_t hash[32];
    bool valid;
};

// GPU Miner interface
class GPUMiner {
public:
    virtual ~GPUMiner() = default;

    // Initialize device
    virtual bool init(int device_id) = 0;

    // Set job parameters
    virtual void set_job(const uint8_t midstate[32],
                         const uint8_t tail[16],
                         const uint8_t target[32]) = 0;

    // Mine a range of nonces
    // Returns number of hashes computed
    virtual uint64_t mine(uint32_t nonce_start, uint32_t nonce_count,
                          std::vector<MiningResult>& results) = 0;

    // Get current hashrate
    virtual double get_hashrate() const = 0;

    // Stop mining
    virtual void stop() = 0;

    // Get device info
    virtual DeviceInfo get_device_info() const = 0;
};

// Factory function type
using GPUMinerFactory = std::unique_ptr<GPUMiner>(*)();

// GPU Manager - handles device enumeration and miner creation
class GPUManager {
public:
    static GPUManager& instance();

    // Enumerate all available GPU devices
    std::vector<DeviceInfo> enumerate_devices();

    // Create a miner for a specific device
    std::unique_ptr<GPUMiner> create_miner(int device_id);

    // Check backend availability
    bool has_cuda() const;
    bool has_opencl() const;

    // Get total GPU count
    unsigned get_device_count() const;

private:
    GPUManager() = default;
    std::vector<DeviceInfo> devices_;
    bool enumerated_ = false;
};

// Callback for when GPU finds a share
using GPUShareCallback = std::function<void(int device_id, const MiningResult& result)>;

// GPU Mining Coordinator - manages multiple GPUs
class GPUCoordinator {
public:
    GPUCoordinator();
    ~GPUCoordinator();

    // Start mining on all available GPUs
    bool start();

    // Stop all GPU mining
    void stop();

    // Set job for all GPUs
    void set_job(const uint8_t midstate[32],
                 const uint8_t tail[16],
                 const uint8_t target[32]);

    // Set callback for shares
    void set_share_callback(GPUShareCallback callback);

    // Get combined hashrate
    double get_total_hashrate() const;

    // Get per-device hashrate
    std::vector<std::pair<int, double>> get_device_hashrates() const;

    // Get device count
    unsigned get_device_count() const;

    // Check if running
    bool is_running() const { return running_.load(); }

private:
    void worker_thread(int device_id);

    std::vector<std::unique_ptr<GPUMiner>> miners_;
    std::vector<std::thread> threads_;
    std::atomic<bool> running_{false};
    std::atomic<bool> has_job_{false};

    // Current job
    uint8_t midstate_[32];
    uint8_t tail_[16];
    uint8_t target_[32];
    std::atomic<uint32_t> nonce_base_{0};

    GPUShareCallback share_callback_;
    mutable std::mutex mutex_;
};

} // namespace gpu
