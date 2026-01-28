// Stratum V1 Protocol Tests

#include "test_framework.hpp"

#include <string>
#include <vector>

namespace test_stratum {

// Stratum parsing functions (from stratum.cpp)
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
                int nest_depth = 1;
                pos++;
                while (pos < json.size() && nest_depth > 0) {
                    if (json[pos] == '[') nest_depth++;
                    else if (json[pos] == ']') nest_depth--;
                    pos++;
                }
                pos--;
            } else if (c == ']') {
                break;
            }
        } else {
            current += c;
        }
        pos++;
    }

    return result;
}

uint64_t parse_block_height(const std::string& coinb1_hex) {
    auto bytes = test::hex_to_bytes(coinb1_hex);
    if (bytes.size() <= 42) return 0;

    size_t script_start = 42;
    uint8_t push_count = bytes[script_start];
    if (push_count < 1 || push_count > 4) return 0;
    if (script_start + 1 + push_count > bytes.size()) return 0;

    uint64_t height = 0;
    for (int i = push_count - 1; i >= 0; i--) {
        height = (height << 8) | bytes[script_start + 1 + i];
    }
    return height;
}

void test_notify_parsing() {
    TEST_GROUP("Stratum Notify Parsing");

    // Empty merkle branch
    {
        std::string msg = R"({"id":null,"method":"mining.notify","params":["job123","prevhash","cb1","cb2",[],"20000000","1a0fffff","12345678",true]})";
        auto params = extract_notify_params(msg);

        bool ok = params.size() >= 7 &&
                  params[0] == "job123" &&
                  params[4] == "20000000" &&
                  params[5] == "1a0fffff" &&
                  params[6] == "12345678";

        TEST_RESULT("Empty merkle branch", ok);
    }

    // Non-empty merkle branch
    {
        std::string msg = R"({"id":null,"method":"mining.notify","params":["job456","prevhash","cb1","cb2",["m1","m2","m3"],"10000000","1b00ffff","aabbccdd",false]})";
        auto params = extract_notify_params(msg);

        bool ok = params.size() >= 7 &&
                  params[0] == "job456" &&
                  params[4] == "10000000" &&
                  params[5] == "1b00ffff" &&
                  params[6] == "aabbccdd";

        TEST_RESULT("With merkle branch", ok);
    }

    // Real ckpool format
    {
        std::string msg = R"({"params":["693b50f000022e1a","4f1c6e6e00000000000000000000000000000000","02000000010000000000000000","ffffffff01",[],"20000000","170b8c8b","678e6b5c",true],"id":null,"method":"mining.notify"})";
        auto params = extract_notify_params(msg);

        bool ok = params.size() >= 7 &&
                  params[0] == "693b50f000022e1a" &&
                  params[4] == "20000000" &&
                  params[5] == "170b8c8b" &&
                  params[6] == "678e6b5c";

        TEST_RESULT("Ckpool format", ok);
    }
}

void test_block_height() {
    TEST_GROUP("Block Height Parsing (BIP34)");

    std::string coinb1_prefix =
        "02000000"  // version
        "01"        // input count
        "0000000000000000000000000000000000000000000000000000000000000000"  // prev txid
        "ffffffff"  // prev vout
        "04";       // script_len

    // Block 880000 (0x0D6D80 LE: 80 6d 0d)
    {
        std::string coinb1 = coinb1_prefix + "03" + "806d0d";
        uint64_t height = parse_block_height(coinb1);
        TEST_RESULT("Block 880000 (3-byte)", height == 880000);
    }

    // Block 100 (1-byte)
    {
        std::string prefix =
            "02000000" "01"
            "0000000000000000000000000000000000000000000000000000000000000000"
            "ffffffff" "02";
        std::string coinb1 = prefix + "01" + "64";
        uint64_t height = parse_block_height(coinb1);
        TEST_RESULT("Block 100 (1-byte)", height == 100);
    }

    // Block 1000 (0x03e8 LE: e8 03)
    {
        std::string prefix =
            "02000000" "01"
            "0000000000000000000000000000000000000000000000000000000000000000"
            "ffffffff" "03";
        std::string coinb1 = prefix + "02" + "e803";
        uint64_t height = parse_block_height(coinb1);
        TEST_RESULT("Block 1000 (2-byte)", height == 1000);
    }

    // Block 16777215 (4-byte max for 3-byte encoding)
    {
        std::string coinb1 = coinb1_prefix + "03" + "ffffff";
        uint64_t height = parse_block_height(coinb1);
        TEST_RESULT("Block 16777215 (3-byte max)", height == 16777215);
    }
}

void test_hex_conversion() {
    TEST_GROUP("Hex Conversion");

    // Valid hex
    {
        auto bytes = test::hex_to_bytes("48656c6c6f");  // "Hello"
        bool ok = bytes.size() == 5 &&
                  bytes[0] == 0x48 && bytes[1] == 0x65 &&
                  bytes[2] == 0x6c && bytes[3] == 0x6c && bytes[4] == 0x6f;
        TEST_RESULT("Valid hex", ok);
    }

    // Empty
    {
        auto bytes = test::hex_to_bytes("");
        TEST_RESULT("Empty string", bytes.empty());
    }

    // Invalid (should not crash)
    {
        auto bytes = test::hex_to_bytes("gg");
        TEST_RESULT("Invalid hex (no crash)", bytes.empty());
    }

    // Odd length
    {
        auto bytes = test::hex_to_bytes("abc");
        TEST_RESULT("Odd length", bytes.size() == 1 && bytes[0] == 0xab);
    }
}

void test_estimated_time() {
    TEST_GROUP("Estimated Time to Share");

    auto estimate = [](double hashrate, double difficulty) -> double {
        if (hashrate <= 0 || difficulty <= 0) return 0;
        return (difficulty * 4294967296.0) / hashrate;
    };

    // 1 MH/s, diff=1 -> ~4295 seconds
    {
        double time = estimate(1e6, 1.0);
        TEST_RESULT("1 MH/s diff=1", time > 4290 && time < 4300);
    }

    // 100 MH/s, diff=1 -> ~43 seconds
    {
        double time = estimate(100e6, 1.0);
        TEST_RESULT("100 MH/s diff=1", time > 42 && time < 44);
    }

    // 100 MH/s, diff=10 -> ~430 seconds
    {
        double time = estimate(100e6, 10.0);
        TEST_RESULT("100 MH/s diff=10", time > 425 && time < 435);
    }

    // Edge cases
    {
        double t1 = estimate(0, 1.0);
        double t2 = estimate(1e6, 0);
        TEST_RESULT("Zero values", t1 == 0 && t2 == 0);
    }
}

void run_all() {
    test_notify_parsing();
    test_block_height();
    test_hex_conversion();
    test_estimated_time();
}

} // namespace test_stratum
