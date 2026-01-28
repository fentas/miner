#pragma once

// CUDA Bitcoin Miner Implementation

#ifdef HAVE_CUDA

#include "gpu.hpp"
#include <cuda_runtime.h>

namespace gpu {

class CUDAMiner : public GPUMiner {
public:
    CUDAMiner();
    ~CUDAMiner() override;

    bool init(int device_id) override;
    void set_job(const uint8_t midstate[32],
                 const uint8_t tail[16],
                 const uint8_t target[32]) override;
    uint64_t mine(uint32_t nonce_start, uint32_t nonce_count,
                  std::vector<MiningResult>& results) override;
    double get_hashrate() const override;
    void stop() override;
    DeviceInfo get_device_info() const override;

private:
    int device_id_ = -1;
    DeviceInfo device_info_;

    // Device memory pointers
    uint32_t* d_midstate_ = nullptr;
    uint8_t* d_tail_ = nullptr;
    uint8_t* d_target_ = nullptr;
    uint32_t* d_nonces_ = nullptr;      // Found nonce results
    uint8_t* d_hashes_ = nullptr;       // Found hash results
    uint32_t* d_count_ = nullptr;       // Number of results found

    // Host memory for results
    static constexpr int MAX_RESULTS = 256;
    uint32_t* h_nonces_ = nullptr;
    uint8_t* h_hashes_ = nullptr;
    uint32_t* h_count_ = nullptr;

    // Hashrate tracking
    std::chrono::steady_clock::time_point start_time_;
    std::atomic<uint64_t> total_hashes_{0};

    // Grid configuration
    int blocks_ = 0;
    int threads_ = 0;

    bool initialized_ = false;
    std::atomic<bool> running_{false};
};

// Check CUDA availability
bool cuda_available();

// Get CUDA device count
int cuda_device_count();

// Get CUDA device info
DeviceInfo cuda_get_device_info(int device_id);

} // namespace gpu

#endif // HAVE_CUDA
