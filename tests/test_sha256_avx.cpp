// AVX2/AVX512 Multi-hashing Tests

#include "test_framework.hpp"
#include "../src/sha256.hpp"
#include "../src/sha256_avx.hpp"

#include <chrono>
#include <iomanip>

namespace test_sha256_avx {

void test_feature_detection() {
    TEST_GROUP("AVX Feature Detection");

#ifdef HAVE_AVX2
    TEST_RESULT("AVX2 compile-time support", sha256::has_avx2());
#else
    TEST_RESULT("AVX2 not compiled", !sha256::has_avx2());
#endif

#ifdef HAVE_AVX512
    TEST_RESULT("AVX512 compile-time support", sha256::has_avx512());
#else
    TEST_RESULT("AVX512 not compiled", !sha256::has_avx512());
#endif

    unsigned batch_size = sha256::get_optimal_batch_size();
    std::cout << "  Optimal batch size: " << batch_size << std::endl;
    TEST_RESULT("Batch size valid", batch_size == 1 || batch_size == 8 || batch_size == 16);
}

#ifdef HAVE_AVX2
void test_avx2_correctness() {
    TEST_GROUP("AVX2 8-way Hashing");

    // Create test midstate
    uint8_t header[80];
    std::memset(header, 0x42, 80);

    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    // Set up easy target (accept most hashes)
    uint8_t target[32];
    std::memset(target, 0xFF, 32);
    target[31] = 0x00;  // Require 1 leading zero byte

    // Compute 8 hashes with AVX2
    uint8_t avx_hashes[8][32];
    uint32_t nonce_base = 12345;
    uint8_t valid_mask = sha256::sha256d_midstate_8way(ms, nonce_base, target, avx_hashes);

    // Verify each hash against scalar implementation
    bool all_correct = true;
    for (int i = 0; i < 8; i++) {
        uint8_t scalar_hash[32];
        sha256::sha256d_midstate(ms, nonce_base + i, scalar_hash);

        if (std::memcmp(avx_hashes[i], scalar_hash, 32) != 0) {
            all_correct = false;
            std::cout << "  Mismatch at lane " << i << std::endl;
        }

        // Verify valid_mask is correct
        bool meets_target = (scalar_hash[31] == 0);
        bool marked_valid = (valid_mask & (1 << i)) != 0;
        if (meets_target != marked_valid) {
            all_correct = false;
            std::cout << "  Target check mismatch at lane " << i << std::endl;
        }
    }

    TEST_RESULT("AVX2 8-way hash correctness", all_correct);

    // Test multiple nonce ranges
    for (uint32_t base = 0; base < 1000000; base += 100000) {
        sha256::sha256d_midstate_8way(ms, base, target, avx_hashes);

        for (int i = 0; i < 8; i++) {
            uint8_t scalar_hash[32];
            sha256::sha256d_midstate(ms, base + i, scalar_hash);
            if (std::memcmp(avx_hashes[i], scalar_hash, 32) != 0) {
                all_correct = false;
            }
        }
    }

    TEST_RESULT("AVX2 8-way multiple ranges", all_correct);
}

void benchmark_avx2() {
    std::cout << "\n[BENCHMARK] AVX2 8-way Performance\n";

    uint8_t header[80];
    std::memset(header, 0x42, 80);

    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    uint8_t target[32];
    std::memset(target, 0xFF, 32);

    uint8_t hashes[8][32];
    const uint32_t iterations = 1000000;  // Total hashes (125K batches of 8)

    auto start = std::chrono::steady_clock::now();
    for (uint32_t i = 0; i < iterations; i += 8) {
        sha256::sha256d_midstate_8way(ms, i, target, hashes);
    }
    auto end = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration<double>(end - start).count();

    double hashrate = iterations / elapsed;

    std::cout << "  " << iterations << " hashes in " << std::fixed
              << std::setprecision(3) << elapsed << "s\n";
    std::cout << "  Hashrate: " << std::fixed << std::setprecision(2);

    if (hashrate >= 1e6) std::cout << (hashrate / 1e6) << " MH/s";
    else if (hashrate >= 1e3) std::cout << (hashrate / 1e3) << " KH/s";
    else std::cout << hashrate << " H/s";
    std::cout << " (single thread, 8-way)\n";
}
#endif

#ifdef HAVE_AVX512
void test_avx512_correctness() {
    TEST_GROUP("AVX512 16-way Hashing");

    uint8_t header[80];
    std::memset(header, 0x42, 80);

    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    uint8_t target[32];
    std::memset(target, 0xFF, 32);
    target[31] = 0x00;

    uint8_t avx_hashes[16][32];
    uint32_t nonce_base = 12345;
    uint16_t valid_mask = sha256::sha256d_midstate_16way(ms, nonce_base, target, avx_hashes);

    bool all_correct = true;
    for (int i = 0; i < 16; i++) {
        uint8_t scalar_hash[32];
        sha256::sha256d_midstate(ms, nonce_base + i, scalar_hash);

        if (std::memcmp(avx_hashes[i], scalar_hash, 32) != 0) {
            all_correct = false;
            std::cout << "  Mismatch at lane " << i << std::endl;
        }

        bool meets_target = (scalar_hash[31] == 0);
        bool marked_valid = (valid_mask & (1 << i)) != 0;
        if (meets_target != marked_valid) {
            all_correct = false;
        }
    }

    TEST_RESULT("AVX512 16-way hash correctness", all_correct);
}

void benchmark_avx512() {
    std::cout << "\n[BENCHMARK] AVX512 16-way Performance\n";

    uint8_t header[80];
    std::memset(header, 0x42, 80);

    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    uint8_t target[32];
    std::memset(target, 0xFF, 32);

    uint8_t hashes[16][32];
    const uint32_t iterations = 1000000;

    auto start = std::chrono::steady_clock::now();
    for (uint32_t i = 0; i < iterations; i += 16) {
        sha256::sha256d_midstate_16way(ms, i, target, hashes);
    }
    auto end = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration<double>(end - start).count();

    double hashrate = iterations / elapsed;

    std::cout << "  " << iterations << " hashes in " << std::fixed
              << std::setprecision(3) << elapsed << "s\n";
    std::cout << "  Hashrate: " << std::fixed << std::setprecision(2);

    if (hashrate >= 1e6) std::cout << (hashrate / 1e6) << " MH/s";
    else if (hashrate >= 1e3) std::cout << (hashrate / 1e3) << " KH/s";
    else std::cout << hashrate << " H/s";
    std::cout << " (single thread, 16-way)\n";
}
#endif

void run_all() {
    test_feature_detection();

#ifdef HAVE_AVX2
    test_avx2_correctness();
    benchmark_avx2();
#else
    std::cout << "\n[SKIP] AVX2 tests (not compiled with AVX2 support)\n";
#endif

#ifdef HAVE_AVX512
    test_avx512_correctness();
    benchmark_avx512();
#else
    std::cout << "\n[SKIP] AVX512 tests (not compiled with AVX512 support)\n";
#endif
}

} // namespace test_sha256_avx
