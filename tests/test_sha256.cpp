// SHA-256 Tests

#include "test_framework.hpp"
#include "../src/sha256.hpp"

#include <chrono>
#include <iomanip>

namespace test_sha256 {

void test_empty_string() {
    TEST_GROUP("SHA-256 Basic");

    // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    uint32_t state[8];
    std::memcpy(state, sha256::H_INIT.data(), 32);

    uint8_t block[64] = {0};
    block[0] = 0x80;

    sha256::transform(state, block);

    uint8_t hash[32];
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        hash[i*4 + 3] = state[i] & 0xff;
    }

    TEST_RESULT("SHA-256 empty string",
        test::compare_hash(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
}

void test_abc() {
    // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    uint32_t state[8];
    std::memcpy(state, sha256::H_INIT.data(), 32);

    uint8_t block[64] = {0};
    block[0] = 'a';
    block[1] = 'b';
    block[2] = 'c';
    block[3] = 0x80;
    block[63] = 0x18;  // 24 bits

    sha256::transform(state, block);

    uint8_t hash[32];
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        hash[i*4 + 3] = state[i] & 0xff;
    }

    TEST_RESULT("SHA-256 'abc'",
        test::compare_hash(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
}

void test_sha256d_hello() {
    TEST_GROUP("SHA-256d (Double Hash)");

    // SHA-256d("hello") = 9595c9df90075148eb06860365df33584b75bff782a510c6cd4883a419833d50
    const char* msg = "hello";
    size_t len = 5;

    uint32_t state[8];
    std::memcpy(state, sha256::H_INIT.data(), 32);

    uint8_t block[64] = {0};
    std::memcpy(block, msg, len);
    block[len] = 0x80;
    block[63] = len * 8;

    sha256::transform(state, block);

    uint8_t first_hash[32];
    for (int i = 0; i < 8; i++) {
        first_hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        first_hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        first_hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        first_hash[i*4 + 3] = state[i] & 0xff;
    }

    // Second hash
    std::memcpy(state, sha256::H_INIT.data(), 32);
    std::memset(block, 0, 64);
    std::memcpy(block, first_hash, 32);
    block[32] = 0x80;
    block[62] = 0x01;  // 256 bits
    block[63] = 0x00;

    sha256::transform(state, block);

    uint8_t hash[32];
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        hash[i*4 + 3] = state[i] & 0xff;
    }

    TEST_RESULT("SHA-256d 'hello'",
        test::compare_hash(hash, "9595c9df90075148eb06860365df33584b75bff782a510c6cd4883a419833d50"));
}

void test_genesis_block() {
    TEST_GROUP("Bitcoin Block Hashing");

    // Genesis block header
    auto header_hex = test::hex_to_bytes(
        "0100000000000000000000000000000000000000000000000000000000000000"
        "000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa"
        "4b1e5e4a29ab5f49ffff001d1dac2b7c"
    );

    uint8_t header[80];
    std::memcpy(header, header_hex.data(), 80);

    uint8_t hash[32];
    sha256::sha256d_80(header, hash);

    // Genesis hash starts with many zeros
    bool valid = (hash[31] == 0x00 && hash[30] == 0x00 &&
                  hash[29] == 0x00 && hash[28] == 0x00);

    TEST_RESULT("Genesis block hash (leading zeros)", valid);
}

void test_midstate() {
    TEST_GROUP("Midstate Optimization");

    uint8_t header[80];
    std::memset(header, 0x42, 80);
    header[76] = header[77] = header[78] = header[79] = 0;

    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    // Test nonce = 0
    uint8_t hash1[32], hash2[32];
    sha256::sha256d_midstate(ms, 0, hash1);
    sha256::sha256d_80(header, hash2);

    TEST_RESULT("Midstate nonce=0", std::memcmp(hash1, hash2, 32) == 0);

    // Test nonce = 0x78563412
    header[76] = 0x12;
    header[77] = 0x34;
    header[78] = 0x56;
    header[79] = 0x78;

    sha256::sha256d_80(header, hash2);
    sha256::sha256d_midstate(ms, 0x78563412, hash1);

    TEST_RESULT("Midstate nonce=0x78563412", std::memcmp(hash1, hash2, 32) == 0);
}

void test_target_comparison() {
    TEST_GROUP("Target Comparison");

    uint8_t hash[32] = {0};
    uint8_t target[32] = {0};

    // hash < target (valid)
    hash[28] = 0x01;
    target[28] = 0x02;

    bool valid = true;
    for (int i = 31; i >= 0; i--) {
        if (hash[i] < target[i]) break;
        if (hash[i] > target[i]) { valid = false; break; }
    }
    TEST_RESULT("hash < target", valid);

    // hash > target (invalid)
    hash[28] = 0x03;
    valid = true;
    for (int i = 31; i >= 0; i--) {
        if (hash[i] < target[i]) break;
        if (hash[i] > target[i]) { valid = false; break; }
    }
    TEST_RESULT("hash > target", !valid);
}

void benchmark() {
    std::cout << "\n[BENCHMARK] SHA-256d Performance\n";

    uint8_t header[80];
    std::memset(header, 0x42, 80);

    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    uint8_t hash[32];
    const uint32_t iterations = 1000000;

    auto start = std::chrono::steady_clock::now();
    for (uint32_t i = 0; i < iterations; i++) {
        sha256::sha256d_midstate(ms, i, hash);
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
    std::cout << " (single thread)\n";

#ifdef HAVE_SHA_NI
    std::cout << "  SHA-NI: enabled\n";
#else
    std::cout << "  SHA-NI: disabled\n";
#endif
}

void run_all() {
    test_empty_string();
    test_abc();
    test_sha256d_hello();
    test_genesis_block();
    test_midstate();
    test_target_comparison();
    benchmark();
}

} // namespace test_sha256
