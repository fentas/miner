#pragma once

// OpenCL Bitcoin Miner Implementation

#ifdef HAVE_OPENCL

#include "gpu.hpp"

#ifdef __APPLE__
#include <OpenCL/opencl.h>
#else
#include <CL/cl.h>
#endif

#include <string>

namespace gpu {

class OpenCLMiner : public GPUMiner {
public:
    OpenCLMiner();
    ~OpenCLMiner() override;

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
    bool build_program();
    std::string get_kernel_source();

    int device_id_ = -1;
    DeviceInfo device_info_;

    // OpenCL objects
    cl_platform_id platform_ = nullptr;
    cl_device_id device_ = nullptr;
    cl_context context_ = nullptr;
    cl_command_queue queue_ = nullptr;
    cl_program program_ = nullptr;
    cl_kernel kernel_ = nullptr;

    // Device memory
    cl_mem d_midstate_ = nullptr;
    cl_mem d_tail_ = nullptr;
    cl_mem d_target_ = nullptr;
    cl_mem d_nonces_ = nullptr;
    cl_mem d_hashes_ = nullptr;
    cl_mem d_count_ = nullptr;

    // Host memory
    static constexpr int MAX_RESULTS = 256;
    uint32_t* h_nonces_ = nullptr;
    uint8_t* h_hashes_ = nullptr;
    uint32_t* h_count_ = nullptr;

    // Hashrate tracking
    std::chrono::steady_clock::time_point start_time_;
    std::atomic<uint64_t> total_hashes_{0};

    // Work configuration
    size_t global_work_size_ = 0;
    size_t local_work_size_ = 0;

    bool initialized_ = false;
    std::atomic<bool> running_{false};
};

// Check OpenCL availability
bool opencl_available();

// Get OpenCL device count
int opencl_device_count();

// Get OpenCL device info
DeviceInfo opencl_get_device_info(int device_id);

// Get all OpenCL platforms and devices
std::vector<std::pair<cl_platform_id, cl_device_id>> opencl_enumerate_devices();

} // namespace gpu

#endif // HAVE_OPENCL
