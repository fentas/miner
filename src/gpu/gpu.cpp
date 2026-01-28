// GPU Mining Implementation

#include "gpu.hpp"
#include <iostream>
#include <thread>
#include <chrono>
#include <cstring>

#ifdef HAVE_CUDA
#include "cuda_miner.cuh"
#endif

#ifdef HAVE_OPENCL
#include "opencl_miner.hpp"
#endif

namespace gpu {

// GPUManager singleton
GPUManager& GPUManager::instance() {
    static GPUManager instance;
    return instance;
}

std::vector<DeviceInfo> GPUManager::enumerate_devices() {
    if (enumerated_) {
        return devices_;
    }

    devices_.clear();

#ifdef HAVE_CUDA
    if (cuda_available()) {
        int count = cuda_device_count();
        for (int i = 0; i < count; i++) {
            DeviceInfo info = cuda_get_device_info(i);
            info.id = static_cast<int>(devices_.size());
            devices_.push_back(info);
        }
    }
#endif

#ifdef HAVE_OPENCL
    if (opencl_available()) {
        int count = opencl_device_count();
        for (int i = 0; i < count; i++) {
            DeviceInfo info = opencl_get_device_info(i);
            info.id = static_cast<int>(devices_.size());
            devices_.push_back(info);
        }
    }
#endif

    enumerated_ = true;
    return devices_;
}

std::unique_ptr<GPUMiner> GPUManager::create_miner(int device_id) {
    if (!enumerated_) {
        enumerate_devices();
    }

    if (device_id < 0 || device_id >= static_cast<int>(devices_.size())) {
        return nullptr;
    }

    const auto& info = devices_[device_id];

#ifdef HAVE_CUDA
    if (info.backend == "cuda") {
        auto miner = std::make_unique<CUDAMiner>();
        if (miner->init(device_id)) {
            return miner;
        }
    }
#endif

#ifdef HAVE_OPENCL
    if (info.backend == "opencl") {
        auto miner = std::make_unique<OpenCLMiner>();
        if (miner->init(device_id)) {
            return miner;
        }
    }
#endif

    return nullptr;
}

bool GPUManager::has_cuda() const {
#ifdef HAVE_CUDA
    return cuda_available();
#else
    return false;
#endif
}

bool GPUManager::has_opencl() const {
#ifdef HAVE_OPENCL
    return opencl_available();
#else
    return false;
#endif
}

unsigned GPUManager::get_device_count() const {
    return static_cast<unsigned>(devices_.size());
}

// GPUCoordinator implementation
GPUCoordinator::GPUCoordinator() {
    std::memset(midstate_, 0, sizeof(midstate_));
    std::memset(tail_, 0, sizeof(tail_));
    std::memset(target_, 0, sizeof(target_));
}

GPUCoordinator::~GPUCoordinator() {
    stop();
}

bool GPUCoordinator::start() {
    if (running_) return true;

    auto& manager = GPUManager::instance();
    auto devices = manager.enumerate_devices();

    if (devices.empty()) {
        std::cerr << "[GPU] No GPU devices found" << std::endl;
        return false;
    }

    std::cout << "[GPU] Found " << devices.size() << " GPU device(s)" << std::endl;

    for (const auto& dev : devices) {
        auto miner = manager.create_miner(dev.id);
        if (miner) {
            std::cout << "[GPU] Initialized: " << dev.name
                      << " (" << dev.backend << ")" << std::endl;
            miners_.push_back(std::move(miner));
        }
    }

    if (miners_.empty()) {
        std::cerr << "[GPU] Failed to initialize any GPU miners" << std::endl;
        return false;
    }

    running_ = true;

    // Start worker threads
    for (size_t i = 0; i < miners_.size(); i++) {
        threads_.emplace_back(&GPUCoordinator::worker_thread, this, static_cast<int>(i));
    }

    return true;
}

void GPUCoordinator::stop() {
    running_ = false;
    has_job_ = false;

    for (auto& miner : miners_) {
        miner->stop();
    }

    for (auto& thread : threads_) {
        if (thread.joinable()) {
            thread.join();
        }
    }

    threads_.clear();
    miners_.clear();
}

void GPUCoordinator::set_job(const uint8_t midstate[32],
                              const uint8_t tail[16],
                              const uint8_t target[32]) {
    std::lock_guard<std::mutex> lock(mutex_);
    std::memcpy(midstate_, midstate, 32);
    std::memcpy(tail_, tail, 16);
    std::memcpy(target_, target, 32);

    for (auto& miner : miners_) {
        miner->set_job(midstate, tail, target);
    }

    nonce_base_ = 0;
    has_job_ = true;
}

void GPUCoordinator::set_share_callback(GPUShareCallback callback) {
    share_callback_ = callback;
}

double GPUCoordinator::get_total_hashrate() const {
    double total = 0;
    for (const auto& miner : miners_) {
        total += miner->get_hashrate();
    }
    return total;
}

std::vector<std::pair<int, double>> GPUCoordinator::get_device_hashrates() const {
    std::vector<std::pair<int, double>> rates;
    for (size_t i = 0; i < miners_.size(); i++) {
        rates.emplace_back(static_cast<int>(i), miners_[i]->get_hashrate());
    }
    return rates;
}

unsigned GPUCoordinator::get_device_count() const {
    return static_cast<unsigned>(miners_.size());
}

void GPUCoordinator::worker_thread(int device_id) {
    constexpr uint32_t NONCE_BATCH = 0x1000000;  // 16M nonces per batch

    while (running_) {
        if (!has_job_) {
            std::this_thread::sleep_for(std::chrono::milliseconds(10));
            continue;
        }

        // Get nonce range
        uint32_t nonce_start = nonce_base_.fetch_add(NONCE_BATCH);

        std::vector<MiningResult> results;
        miners_[device_id]->mine(nonce_start, NONCE_BATCH, results);

        // Process results
        for (const auto& result : results) {
            if (result.valid && share_callback_) {
                share_callback_(device_id, result);
            }
        }
    }
}

// Stub implementations for when backends are not available

#ifndef HAVE_CUDA
bool cuda_available() { return false; }
int cuda_device_count() { return 0; }
DeviceInfo cuda_get_device_info(int) { return DeviceInfo{}; }
#endif

#ifndef HAVE_OPENCL
bool opencl_available() { return false; }
int opencl_device_count() { return 0; }
DeviceInfo opencl_get_device_info(int) { return DeviceInfo{}; }
std::vector<std::pair<void*, void*>> opencl_enumerate_devices() { return {}; }
#endif

} // namespace gpu
