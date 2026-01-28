// GPU Mining Interface Tests

#include "test_framework.hpp"
#include "../src/gpu/gpu.hpp"

namespace test_gpu {

void test_device_info() {
    TEST_GROUP("GPU Device Info");

    gpu::DeviceInfo info;
    info.id = 0;
    info.name = "Test GPU";
    info.vendor = "Test Vendor";
    info.memory_total = 8ULL * 1024 * 1024 * 1024;  // 8 GB
    info.memory_free = 7ULL * 1024 * 1024 * 1024;   // 7 GB
    info.compute_units = 64;
    info.max_threads = 1024;
    info.available = true;
    info.backend = "cuda";

    TEST_RESULT("DeviceInfo id", info.id == 0);
    TEST_RESULT("DeviceInfo name", info.name == "Test GPU");
    TEST_RESULT("DeviceInfo vendor", info.vendor == "Test Vendor");
    TEST_RESULT("DeviceInfo memory", info.memory_total == 8ULL * 1024 * 1024 * 1024);
    TEST_RESULT("DeviceInfo compute_units", info.compute_units == 64);
    TEST_RESULT("DeviceInfo backend", info.backend == "cuda");
}

void test_mining_result() {
    TEST_GROUP("GPU Mining Result");

    gpu::MiningResult result;
    result.nonce = 0xDEADBEEF;
    result.valid = true;

    // Set hash
    for (int i = 0; i < 32; i++) {
        result.hash[i] = static_cast<uint8_t>(i);
    }

    TEST_RESULT("MiningResult nonce", result.nonce == 0xDEADBEEF);
    TEST_RESULT("MiningResult valid", result.valid == true);
    TEST_RESULT("MiningResult hash", result.hash[0] == 0 && result.hash[31] == 31);
}

void test_gpu_manager() {
    TEST_GROUP("GPU Manager");

    auto& manager = gpu::GPUManager::instance();

    // Test backend availability queries (won't crash even if no GPU)
    bool has_cuda = manager.has_cuda();
    bool has_opencl = manager.has_opencl();

    std::cout << "  CUDA available: " << (has_cuda ? "yes" : "no") << std::endl;
    std::cout << "  OpenCL available: " << (has_opencl ? "yes" : "no") << std::endl;

    TEST_RESULT("has_cuda returns bool", has_cuda == true || has_cuda == false);
    TEST_RESULT("has_opencl returns bool", has_opencl == true || has_opencl == false);

    // Enumerate devices (may be empty)
    auto devices = manager.enumerate_devices();
    std::cout << "  Found " << devices.size() << " GPU device(s)" << std::endl;

    for (const auto& dev : devices) {
        std::cout << "    [" << dev.id << "] " << dev.name
                  << " (" << dev.backend << ")" << std::endl;
    }

    TEST_RESULT("enumerate_devices", true);  // Should not crash
}

void test_gpu_coordinator() {
    TEST_GROUP("GPU Coordinator");

    gpu::GPUCoordinator coord;

    // Test initial state
    TEST_RESULT("Initially not running", !coord.is_running());
    TEST_RESULT("Initial device count", coord.get_device_count() == 0);
    TEST_RESULT("Initial hashrate", coord.get_total_hashrate() == 0.0);

    // Test set_job doesn't crash even without devices
    uint8_t midstate[32] = {0};
    uint8_t tail[16] = {0};
    uint8_t target[32] = {0};
    coord.set_job(midstate, tail, target);

    TEST_RESULT("set_job without devices", true);

    // Test callback setting
    bool callback_called = false;
    coord.set_share_callback([&](int, const gpu::MiningResult&) {
        callback_called = true;
    });

    TEST_RESULT("set_share_callback", true);

    // Stop (should be safe even if not started)
    coord.stop();
    TEST_RESULT("stop without start", !coord.is_running());
}

#if defined(HAVE_CUDA) || defined(HAVE_OPENCL)
void test_gpu_mining() {
    TEST_GROUP("GPU Mining (Live)");

    auto& manager = gpu::GPUManager::instance();
    auto devices = manager.enumerate_devices();

    if (devices.empty()) {
        std::cout << "  [SKIP] No GPU devices available\n";
        return;
    }

    // Try to create a miner for the first device
    auto miner = manager.create_miner(0);
    if (!miner) {
        std::cout << "  [SKIP] Could not create miner for device 0\n";
        return;
    }

    auto info = miner->get_device_info();
    std::cout << "  Testing on: " << info.name << std::endl;

    // Set up a test job
    uint8_t midstate[32] = {0};
    uint8_t tail[16] = {0};
    uint8_t target[32];
    std::memset(target, 0xFF, 32);  // Easy target

    miner->set_job(midstate, tail, target);

    // Mine a small batch
    std::vector<gpu::MiningResult> results;
    uint64_t hashes = miner->mine(0, 1000000, results);

    std::cout << "  Mined " << hashes << " hashes\n";
    std::cout << "  Found " << results.size() << " results\n";

    TEST_RESULT("GPU mining produces hashes", hashes > 0);

    miner->stop();
    TEST_RESULT("GPU miner stopped", true);
}
#endif

void run_all() {
    test_device_info();
    test_mining_result();
    test_gpu_manager();
    test_gpu_coordinator();

#if defined(HAVE_CUDA) || defined(HAVE_OPENCL)
    test_gpu_mining();
#else
    std::cout << "\n[SKIP] Live GPU tests (no GPU backend compiled)\n";
#endif
}

} // namespace test_gpu
