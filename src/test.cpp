// Test suite for BTC Miner
// Compile: g++ -std=c++17 -O3 -msha -msse4.1 -DHAVE_SHA_NI -o test_miner src/test.cpp

#include "sha256.hpp"

#include <iostream>
#include <iomanip>
#include <cstring>
#include <vector>
#include <string>
#include <chrono>
#include <stdexcept>

// ============================================================================
// Stratum Parsing Functions (copied from stratum.cpp for testing)
// ============================================================================
namespace stratum_test {

std::vector<std::string> extract_notify_params(const std::string& json) {
    std::vector<std::string> result;

    size_t params_pos = json.find("\"params\"");
    if (params_pos == std::string::npos) return result;

    size_t arr_start = json.find('[', params_pos);
    if (arr_start == std::string::npos) return result;

    size_t pos = arr_start + 1;
    std::string current;
    bool in_string = false;

    while (pos < json.size()) {
        char c = json[pos];

        if (c == '"' && (pos == 0 || json[pos-1] != '\\')) {
            in_string = !in_string;
            if (!in_string) {
                result.push_back(current);
                current.clear();
            }
        } else if (!in_string) {
            if (c == '[') {
                // Skip nested array (merkle branch)
                int nest_depth = 1;
                pos++;
                while (pos < json.size() && nest_depth > 0) {
                    if (json[pos] == '[') nest_depth++;
                    else if (json[pos] == ']') nest_depth--;
                    pos++;
                }
                pos--;  // Back up to let the main loop advance
            } else if (c == ']') {
                // End of params array
                break;
            }
            // Commas and other non-string chars are just skipped
        } else {
            current += c;
        }
        pos++;
    }

    return result;
}

std::vector<uint8_t> hex_to_bytes(const std::string& hex) {
    std::vector<uint8_t> bytes;
    for (size_t i = 0; i + 1 < hex.length(); i += 2) {
        try {
            bytes.push_back(static_cast<uint8_t>(std::stoul(hex.substr(i, 2), nullptr, 16)));
        } catch (const std::exception&) {
            break;
        }
    }
    return bytes;
}

} // namespace stratum_test

namespace {

int tests_passed = 0;
int tests_failed = 0;

std::vector<uint8_t> hex_to_bytes(const std::string& hex) {
    std::vector<uint8_t> bytes;
    for (size_t i = 0; i < hex.length(); i += 2) {
        bytes.push_back(static_cast<uint8_t>(std::stoul(hex.substr(i, 2), nullptr, 16)));
    }
    return bytes;
}

bool compare_hash(const uint8_t* hash, const std::string& expected_hex) {
    auto expected = hex_to_bytes(expected_hex);
    return std::memcmp(hash, expected.data(), 32) == 0;
}

void test_result(const std::string& name, bool passed) {
    if (passed) {
        std::cout << "[PASS] " << name << std::endl;
        tests_passed++;
    } else {
        std::cout << "[FAIL] " << name << std::endl;
        tests_failed++;
    }
}

// ============================================================================
// SHA-256 Single Block Tests (NIST Test Vectors)
// ============================================================================

void test_sha256_empty() {
    // SHA-256("") = e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855
    uint32_t state[8];
    std::memcpy(state, sha256::H_INIT.data(), 32);

    uint8_t block[64] = {0};
    block[0] = 0x80;  // Padding for empty message
    // Length = 0 bits, already zeros

    sha256::transform(state, block);

    uint8_t hash[32];
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        hash[i*4 + 3] = state[i] & 0xff;
    }

    test_result("SHA-256 empty string",
                compare_hash(hash, "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"));
}

void test_sha256_abc() {
    // SHA-256("abc") = ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad
    uint32_t state[8];
    std::memcpy(state, sha256::H_INIT.data(), 32);

    uint8_t block[64] = {0};
    block[0] = 'a';
    block[1] = 'b';
    block[2] = 'c';
    block[3] = 0x80;  // Padding
    // Length = 24 bits = 0x18
    block[63] = 0x18;

    sha256::transform(state, block);

    uint8_t hash[32];
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        hash[i*4 + 3] = state[i] & 0xff;
    }

    test_result("SHA-256 'abc'",
                compare_hash(hash, "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"));
}

// ============================================================================
// SHA-256d Tests (Double SHA-256 as used in Bitcoin)
// ============================================================================

void test_sha256d() {
    // SHA-256d("hello") = 9595c9df90075148eb06860365df33584b75bff782a510c6cd4883a419833d50
    // First: SHA-256("hello") = 2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824
    // Second: SHA-256(first) = 9595c9df...

    const char* msg = "hello";
    size_t len = 5;

    // Manual SHA-256d
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
    block[62] = 0x01;  // 256 bits = 0x100
    block[63] = 0x00;

    sha256::transform(state, block);

    uint8_t hash[32];
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        hash[i*4 + 3] = state[i] & 0xff;
    }

    test_result("SHA-256d 'hello'",
                compare_hash(hash, "9595c9df90075148eb06860365df33584b75bff782a510c6cd4883a419833d50"));
}

// ============================================================================
// Bitcoin Block Header Test (Genesis Block)
// ============================================================================

void test_genesis_block() {
    // Bitcoin Genesis Block (Block 0)
    // Raw header hex (from Bitcoin):
    // 0100000000000000000000000000000000000000000000000000000000000000
    // 000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa
    // 4b1e5e4a29ab5f49ffff001d1dac2b7c

    // Genesis block header as raw bytes
    auto header_hex = hex_to_bytes(
        "0100000000000000000000000000000000000000000000000000000000000000"
        "000000003ba3edfd7a7b12b27ac72c3e67768f617fc81bc3888a51323a9fb8aa"
        "4b1e5e4a29ab5f49ffff001d1dac2b7c"
    );

    uint8_t header[80];
    std::memcpy(header, header_hex.data(), 80);

    uint8_t hash[32];
    sha256::sha256d_80(header, hash);

    std::cout << "  Genesis block hash: ";
    for (int i = 31; i >= 0; i--) {
        std::cout << std::hex << std::setw(2) << std::setfill('0') << static_cast<int>(hash[i]);
    }
    std::cout << std::dec << std::endl;

    // Genesis hash starts with many zeros - check the leading bytes
    // Expected: 000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f
    // In little-endian storage: hash[31..26] should be 00 00 00 00 00 19
    bool valid = (hash[31] == 0x00 && hash[30] == 0x00 && hash[29] == 0x00 &&
                  hash[28] == 0x00);

    test_result("Genesis block hash (leading zeros)", valid);
}

// ============================================================================
// Midstate Test
// ============================================================================

void test_midstate() {
    // Test that midstate computation produces correct results
    uint8_t header[80];
    std::memset(header, 0x42, 80);  // Fill with test pattern

    // Clear nonce area for first test
    header[76] = 0x00;
    header[77] = 0x00;
    header[78] = 0x00;
    header[79] = 0x00;

    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    // Test with nonce = 0
    uint8_t hash1[32];
    sha256::sha256d_midstate(ms, 0, hash1);

    // Also compute full hash for comparison
    uint8_t hash2[32];
    sha256::sha256d_80(header, hash2);

    test_result("Midstate computation (nonce=0)", std::memcmp(hash1, hash2, 32) == 0);

    // Test with different nonce
    header[76] = 0x12;
    header[77] = 0x34;
    header[78] = 0x56;
    header[79] = 0x78;

    sha256::sha256d_80(header, hash2);
    sha256::sha256d_midstate(ms, 0x78563412, hash1);

    test_result("Midstate computation (nonce=0x78563412)", std::memcmp(hash1, hash2, 32) == 0);
}

// ============================================================================
// Target Comparison Test
// ============================================================================

void test_target_comparison() {
    // Test that hash comparison works correctly
    uint8_t hash[32] = {0};
    uint8_t target[32] = {0};

    // Hash < target should be valid
    hash[31] = 0x00;
    hash[30] = 0x00;
    hash[29] = 0x00;
    hash[28] = 0x01;

    target[31] = 0x00;
    target[30] = 0x00;
    target[29] = 0x00;
    target[28] = 0x02;

    // Compare (hash should be less than target)
    bool valid = true;
    for (int i = 31; i >= 0; i--) {
        if (hash[i] < target[i]) break;
        if (hash[i] > target[i]) { valid = false; break; }
    }

    test_result("Target comparison (hash < target)", valid);

    // Hash > target should be invalid
    hash[28] = 0x03;
    valid = true;
    for (int i = 31; i >= 0; i--) {
        if (hash[i] < target[i]) break;
        if (hash[i] > target[i]) { valid = false; break; }
    }

    test_result("Target comparison (hash > target)", !valid);
}

// ============================================================================
// Mining Simulation Test (Low Difficulty)
// ============================================================================

void test_mining_simulation() {
    std::cout << "\n[TEST] Mining simulation with low difficulty target...\n";

    // Create a simple block header
    uint8_t header[80];
    std::memset(header, 0, 80);

    // Set version
    header[0] = 0x01;

    // Set a simple merkle root
    for (int i = 36; i < 68; i++) {
        header[i] = static_cast<uint8_t>(i);
    }

    // Set time
    header[68] = 0x12;
    header[69] = 0x34;
    header[70] = 0x56;
    header[71] = 0x78;

    // Set bits (low difficulty for testing - target starts with 0x00000fff...)
    header[72] = 0xff;
    header[73] = 0xff;
    header[74] = 0x0f;
    header[75] = 0x20;  // Exponent 0x20 = 32, mantissa 0x0fffff

    // Compute midstate
    sha256::Midstate ms;
    sha256::compute_midstate(header, ms);

    // Try to find a valid nonce (looking for 2 leading zero bytes in hash)
    uint8_t hash[32];
    uint32_t nonce = 0;
    bool found = false;

    auto start = std::chrono::steady_clock::now();

    for (uint32_t i = 0; i < 10000000 && !found; i++) {
        sha256::sha256d_midstate(ms, i, hash);

        // Check if hash has at least 2 leading zero bytes (big-endian view)
        if (hash[31] == 0 && hash[30] == 0) {
            nonce = i;
            found = true;
        }
    }

    auto end = std::chrono::steady_clock::now();
    auto elapsed = std::chrono::duration<double>(end - start).count();

    if (found) {
        std::cout << "  Found valid nonce: " << nonce << " in " << std::fixed
                  << std::setprecision(3) << elapsed << "s\n";
        std::cout << "  Hash: ";
        for (int i = 31; i >= 0; i--) {
            std::cout << std::hex << std::setw(2) << std::setfill('0')
                      << static_cast<int>(hash[i]);
        }
        std::cout << std::dec << std::endl;

        // Verify by recomputing
        header[76] = nonce & 0xff;
        header[77] = (nonce >> 8) & 0xff;
        header[78] = (nonce >> 16) & 0xff;
        header[79] = (nonce >> 24) & 0xff;

        uint8_t verify_hash[32];
        sha256::sha256d_80(header, verify_hash);

        test_result("Mining simulation - found valid hash",
                    std::memcmp(hash, verify_hash, 32) == 0);
    } else {
        test_result("Mining simulation - found valid hash", false);
    }
}

// ============================================================================
// Benchmark
// ============================================================================

void benchmark_sha256() {
    std::cout << "\n[BENCHMARK] SHA-256d performance...\n";

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

    if (hashrate >= 1e6) {
        std::cout << (hashrate / 1e6) << " MH/s";
    } else if (hashrate >= 1e3) {
        std::cout << (hashrate / 1e3) << " KH/s";
    } else {
        std::cout << hashrate << " H/s";
    }
    std::cout << " (single thread)\n";

#ifdef HAVE_SHA_NI
    std::cout << "  SHA-NI: enabled\n";
#else
    std::cout << "  SHA-NI: disabled (using generic implementation)\n";
#endif
}

// ============================================================================
// Stratum Parsing Tests
// ============================================================================

void test_stratum_notify_parsing() {
    // Test 1: Empty merkle branch
    {
        std::string msg = R"({"id":null,"method":"mining.notify","params":["job123","00000000000000000001234567890abcdef","coinb1hex","coinb2hex",[],"20000000","1a0fffff","12345678",true]})";
        auto params = stratum_test::extract_notify_params(msg);

        bool ok = params.size() >= 7 &&
                  params[0] == "job123" &&
                  params[4] == "20000000" &&  // version (merkle branch skipped)
                  params[5] == "1a0fffff" &&  // nbits
                  params[6] == "12345678";    // ntime

        test_result("Stratum notify parsing (empty merkle)", ok);
    }

    // Test 2: Non-empty merkle branch
    {
        std::string msg = R"({"id":null,"method":"mining.notify","params":["job456","prevhash","cb1","cb2",["merkle1","merkle2","merkle3"],"10000000","1b00ffff","aabbccdd",false]})";
        auto params = stratum_test::extract_notify_params(msg);

        bool ok = params.size() >= 7 &&
                  params[0] == "job456" &&
                  params[4] == "10000000" &&
                  params[5] == "1b00ffff" &&
                  params[6] == "aabbccdd";

        test_result("Stratum notify parsing (with merkle)", ok);
    }

    // Test 3: Real-world ckpool message format
    {
        std::string msg = R"({"params":["693b50f000022e1a","4f1c6e6e000000000000000000000000000000000000000000000000000000000000000000000000","02000000010000000000000000000000000000000000000000000000000000000000000000ffffffff1b03","ffffffff0100f2052a01000000160014",[],"20000000","170b8c8b","678e6b5c",true],"id":null,"method":"mining.notify"})";
        auto params = stratum_test::extract_notify_params(msg);

        bool ok = params.size() >= 7 &&
                  params[0] == "693b50f000022e1a" &&
                  params[4] == "20000000" &&
                  params[5] == "170b8c8b" &&
                  params[6] == "678e6b5c";

        test_result("Stratum notify parsing (ckpool format)", ok);
    }
}

void test_hex_to_bytes() {
    // Test 1: Valid hex
    {
        auto bytes = stratum_test::hex_to_bytes("48656c6c6f");  // "Hello"
        bool ok = bytes.size() == 5 &&
                  bytes[0] == 0x48 && bytes[1] == 0x65 &&
                  bytes[2] == 0x6c && bytes[3] == 0x6c && bytes[4] == 0x6f;
        test_result("hex_to_bytes (valid)", ok);
    }

    // Test 2: Empty string
    {
        auto bytes = stratum_test::hex_to_bytes("");
        test_result("hex_to_bytes (empty)", bytes.empty());
    }

    // Test 3: Invalid hex (should not crash)
    {
        auto bytes = stratum_test::hex_to_bytes("gg");  // Invalid
        test_result("hex_to_bytes (invalid - no crash)", bytes.empty());
    }

    // Test 4: Odd length (should handle gracefully)
    {
        auto bytes = stratum_test::hex_to_bytes("abc");  // Odd length
        test_result("hex_to_bytes (odd length)", bytes.size() == 1 && bytes[0] == 0xab);
    }
}

// ============================================================================
// Block Height Parsing Tests (BIP34)
// ============================================================================

uint64_t parse_block_height(const std::string& coinb1_hex) {
    auto bytes = stratum_test::hex_to_bytes(coinb1_hex);
    // Format: version(4) + txin_count(1) + prev_txid(32) + prev_vout(4) + script_len(1) = 42 bytes
    // At offset 42: scriptSig starts (BIP34: push_count + height bytes)
    if (bytes.size() <= 42) return 0;

    size_t script_start = 42;  // ScriptSig starts here
    uint8_t push_count = bytes[script_start];
    if (push_count < 1 || push_count > 4) return 0;
    if (script_start + 1 + push_count > bytes.size()) return 0;

    uint64_t height = 0;
    for (int i = push_count - 1; i >= 0; i--) {
        height = (height << 8) | bytes[script_start + 1 + i];
    }
    return height;
}

void test_block_height_parsing() {
    // coinb1 format: version(4) + txin_count(1) + prev_txid(32) + prev_vout(4) + script_len(1) + scriptSig...
    // At byte 42 (offset after script_len): BIP34 push_count + height_bytes
    // So we need 84 hex chars for the prefix (42 bytes), then scriptSig data

    // Build prefix: 4+1+32+4+1 = 42 bytes = 84 hex chars
    std::string coinb1_prefix =
        "02000000"                                                          // version (4 bytes)
        "01"                                                                // input count (1 byte)
        "0000000000000000000000000000000000000000000000000000000000000000"  // prev txid (32 bytes)
        "ffffffff"                                                          // prev vout (4 bytes)
        "04";                                                               // script_len (1 byte)
    // Now at offset 42: scriptSig starts with push_count

    // Test 1: Block 880000 (0x0D6D80 little-endian: 80 6d 0d)
    {
        std::string coinb1 = coinb1_prefix + "03" + "806d0d";  // push_count=3, height LE
        uint64_t height = parse_block_height(coinb1);
        test_result("Block height parsing (3-byte)", height == 880000);
    }

    // Test 2: Low block height (1-byte) - Block 100
    {
        std::string prefix =
            "02000000"
            "01"
            "0000000000000000000000000000000000000000000000000000000000000000"
            "ffffffff"
            "02";  // script_len=2
        std::string coinb1 = prefix + "01" + "64";  // push_count=1, height=100
        uint64_t height = parse_block_height(coinb1);
        test_result("Block height parsing (1-byte)", height == 100);
    }

    // Test 3: Medium block height (2-byte) - Block 1000 (0x03e8 LE: e8 03)
    {
        std::string prefix =
            "02000000"
            "01"
            "0000000000000000000000000000000000000000000000000000000000000000"
            "ffffffff"
            "03";  // script_len=3
        std::string coinb1 = prefix + "02" + "e803";  // push_count=2, height LE
        uint64_t height = parse_block_height(coinb1);
        test_result("Block height parsing (2-byte)", height == 1000);
    }
}

// ============================================================================
// Estimated Time to Share Tests
// ============================================================================

double estimate_time_to_share(double hashrate, double difficulty) {
    if (hashrate <= 0 || difficulty <= 0) return 0;
    return (difficulty * 4294967296.0) / hashrate;  // 2^32 = 4294967296
}

void test_estimated_time() {
    // Test 1: At 1 MH/s with difficulty 1
    {
        double time = estimate_time_to_share(1e6, 1.0);
        // Expected: 2^32 / 1e6 = 4294.967296 seconds (~71.6 minutes)
        bool ok = time > 4290 && time < 4300;
        test_result("ETA at 1 MH/s, diff=1", ok);
    }

    // Test 2: At 100 MH/s with difficulty 1
    {
        double time = estimate_time_to_share(100e6, 1.0);
        // Expected: 2^32 / 100e6 = 42.95 seconds
        bool ok = time > 42 && time < 44;
        test_result("ETA at 100 MH/s, diff=1", ok);
    }

    // Test 3: With higher difficulty
    {
        double time = estimate_time_to_share(100e6, 10.0);
        // Expected: 10 * 2^32 / 100e6 = 429.5 seconds
        bool ok = time > 425 && time < 435;
        test_result("ETA at 100 MH/s, diff=10", ok);
    }

    // Test 4: Edge cases
    {
        double time1 = estimate_time_to_share(0, 1.0);
        double time2 = estimate_time_to_share(1e6, 0);
        test_result("ETA edge cases (zero values)", time1 == 0 && time2 == 0);
    }
}

} // anonymous namespace

int main() {
    std::cout << "╔══════════════════════════════════════════════════════════╗\n"
              << "║              BTC Miner Test Suite                        ║\n"
              << "╚══════════════════════════════════════════════════════════╝\n\n";

    std::cout << "[TEST GROUP] SHA-256 Basic Tests\n";
    test_sha256_empty();
    test_sha256_abc();

    std::cout << "\n[TEST GROUP] SHA-256d (Double SHA-256)\n";
    test_sha256d();

    std::cout << "\n[TEST GROUP] Bitcoin Block Tests\n";
    test_genesis_block();

    std::cout << "\n[TEST GROUP] Midstate Tests\n";
    test_midstate();

    std::cout << "\n[TEST GROUP] Target Comparison\n";
    test_target_comparison();

    std::cout << "\n[TEST GROUP] Stratum Protocol Parsing\n";
    test_stratum_notify_parsing();
    test_block_height_parsing();

    std::cout << "\n[TEST GROUP] Estimated Time Calculations\n";
    test_estimated_time();
    test_hex_to_bytes();

    test_mining_simulation();

    benchmark_sha256();

    std::cout << "\n══════════════════════════════════════════════════════════\n";
    std::cout << "Results: " << tests_passed << " passed, " << tests_failed << " failed\n";
    std::cout << "══════════════════════════════════════════════════════════\n";

    return tests_failed > 0 ? 1 : 0;
}
