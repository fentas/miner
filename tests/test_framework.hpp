#pragma once

// Minimal test framework

#include <iostream>
#include <string>
#include <vector>
#include <functional>
#include <cstring>

namespace test {

struct TestResult {
    std::string name;
    bool passed;
    std::string message;
};

class TestSuite {
public:
    static TestSuite& instance() {
        static TestSuite inst;
        return inst;
    }

    void add_result(const std::string& name, bool passed, const std::string& msg = "") {
        results_.push_back({name, passed, msg});
        if (passed) {
            passed_++;
            std::cout << "[PASS] " << name << std::endl;
        } else {
            failed_++;
            std::cout << "[FAIL] " << name;
            if (!msg.empty()) std::cout << " - " << msg;
            std::cout << std::endl;
        }
    }

    void group(const std::string& name) {
        std::cout << "\n[TEST GROUP] " << name << std::endl;
    }

    int passed() const { return passed_; }
    int failed() const { return failed_; }
    int total() const { return passed_ + failed_; }

    void summary() const {
        std::cout << "\n══════════════════════════════════════════════════════════\n";
        std::cout << "Results: " << passed_ << " passed, " << failed_ << " failed\n";
        std::cout << "══════════════════════════════════════════════════════════\n";
    }

private:
    TestSuite() = default;
    std::vector<TestResult> results_;
    int passed_ = 0;
    int failed_ = 0;
};

#define TEST(name) \
    void test_##name(); \
    struct TestRegister_##name { \
        TestRegister_##name() { test_##name(); } \
    }; \
    void test_##name()

#define EXPECT_TRUE(expr) \
    test::TestSuite::instance().add_result(#expr, (expr))

#define EXPECT_FALSE(expr) \
    test::TestSuite::instance().add_result(#expr " == false", !(expr))

#define EXPECT_EQ(a, b) \
    test::TestSuite::instance().add_result(#a " == " #b, (a) == (b))

#define EXPECT_NE(a, b) \
    test::TestSuite::instance().add_result(#a " != " #b, (a) != (b))

#define EXPECT_LT(a, b) \
    test::TestSuite::instance().add_result(#a " < " #b, (a) < (b))

#define EXPECT_GT(a, b) \
    test::TestSuite::instance().add_result(#a " > " #b, (a) > (b))

#define EXPECT_NEAR(a, b, eps) \
    test::TestSuite::instance().add_result(#a " ~= " #b, std::abs((a) - (b)) < (eps))

#define TEST_GROUP(name) \
    test::TestSuite::instance().group(name)

#define TEST_RESULT(name, passed) \
    test::TestSuite::instance().add_result(name, passed)

// Utility functions
inline std::vector<uint8_t> hex_to_bytes(const std::string& hex) {
    std::vector<uint8_t> bytes;
    for (size_t i = 0; i + 1 < hex.length(); i += 2) {
        try {
            bytes.push_back(static_cast<uint8_t>(std::stoul(hex.substr(i, 2), nullptr, 16)));
        } catch (...) {
            break;
        }
    }
    return bytes;
}

inline std::string bytes_to_hex(const uint8_t* data, size_t len) {
    std::string hex;
    hex.reserve(len * 2);
    const char* digits = "0123456789abcdef";
    for (size_t i = 0; i < len; i++) {
        hex.push_back(digits[data[i] >> 4]);
        hex.push_back(digits[data[i] & 0xf]);
    }
    return hex;
}

inline bool compare_bytes(const uint8_t* a, const uint8_t* b, size_t len) {
    return std::memcmp(a, b, len) == 0;
}

inline bool compare_hash(const uint8_t* hash, const std::string& expected_hex) {
    auto expected = hex_to_bytes(expected_hex);
    return compare_bytes(hash, expected.data(), 32);
}

} // namespace test
