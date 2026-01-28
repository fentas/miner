// Stratum V2 Protocol Tests

#include "test_framework.hpp"
#include "../src/stratum_v2.hpp"

#include <cstring>

namespace test_stratum_v2 {

void test_byte_buffer_write() {
    TEST_GROUP("Stratum V2 ByteBuffer Write");

    stratum_v2::ByteBuffer buf;

    // Write u8
    buf.write_u8(0x42);
    TEST_RESULT("write_u8", buf.size() == 1 && buf.data()[0] == 0x42);

    // Write u16 (little-endian)
    buf.clear();
    buf.write_u16(0x1234);
    TEST_RESULT("write_u16", buf.size() == 2 &&
        buf.data()[0] == 0x34 && buf.data()[1] == 0x12);

    // Write u32 (little-endian)
    buf.clear();
    buf.write_u32(0x12345678);
    TEST_RESULT("write_u32", buf.size() == 4 &&
        buf.data()[0] == 0x78 && buf.data()[1] == 0x56 &&
        buf.data()[2] == 0x34 && buf.data()[3] == 0x12);

    // Write u64 (little-endian)
    buf.clear();
    buf.write_u64(0x123456789ABCDEF0ULL);
    TEST_RESULT("write_u64", buf.size() == 8 &&
        buf.data()[0] == 0xF0 && buf.data()[7] == 0x12);

    // Write string
    buf.clear();
    buf.write_string("hello");
    TEST_RESULT("write_string", buf.size() == 6 &&
        buf.data()[0] == 5 &&  // length byte
        buf.data()[1] == 'h' && buf.data()[5] == 'o');

    // Write bool
    buf.clear();
    buf.write_bool(true);
    buf.write_bool(false);
    TEST_RESULT("write_bool", buf.size() == 2 &&
        buf.data()[0] == 1 && buf.data()[1] == 0);
}

void test_byte_buffer_read() {
    TEST_GROUP("Stratum V2 ByteBuffer Read");

    // Prepare buffer
    std::vector<uint8_t> data = {
        0x42,                   // u8
        0x34, 0x12,             // u16
        0x78, 0x56, 0x34, 0x12, // u32
        0x05, 'h', 'e', 'l', 'l', 'o',  // string
        0x01, 0x00              // bools
    };

    stratum_v2::ByteBuffer buf(data);

    TEST_RESULT("read_u8", buf.read_u8() == 0x42);
    TEST_RESULT("read_u16", buf.read_u16() == 0x1234);
    TEST_RESULT("read_u32", buf.read_u32() == 0x12345678);
    TEST_RESULT("read_string", buf.read_string() == "hello");
    TEST_RESULT("read_bool true", buf.read_bool() == true);
    TEST_RESULT("read_bool false", buf.read_bool() == false);
}

void test_byte_buffer_roundtrip() {
    TEST_GROUP("Stratum V2 ByteBuffer Roundtrip");

    stratum_v2::ByteBuffer write_buf;

    // Write various types
    write_buf.write_u8(0xFF);
    write_buf.write_u16(0xABCD);
    write_buf.write_u32(0xDEADBEEF);
    write_buf.write_u64(0x0102030405060708ULL);
    write_buf.write_string("test string");
    write_buf.write_bool(true);

    // Read back
    stratum_v2::ByteBuffer read_buf(write_buf.buffer());

    TEST_RESULT("roundtrip u8", read_buf.read_u8() == 0xFF);
    TEST_RESULT("roundtrip u16", read_buf.read_u16() == 0xABCD);
    TEST_RESULT("roundtrip u32", read_buf.read_u32() == 0xDEADBEEF);
    TEST_RESULT("roundtrip u64", read_buf.read_u64() == 0x0102030405060708ULL);
    TEST_RESULT("roundtrip string", read_buf.read_string() == "test string");
    TEST_RESULT("roundtrip bool", read_buf.read_bool() == true);
    TEST_RESULT("buffer consumed", read_buf.remaining() == 0);
}

void test_message_types() {
    TEST_GROUP("Stratum V2 Message Types");

    // Verify enum values match spec
    TEST_RESULT("OpenStandardMiningChannel",
        static_cast<uint8_t>(stratum_v2::MessageType::OpenStandardMiningChannel) == 0x00);
    TEST_RESULT("NewMiningJob",
        static_cast<uint8_t>(stratum_v2::MessageType::NewMiningJob) == 0x11);
    TEST_RESULT("SubmitSharesStandard",
        static_cast<uint8_t>(stratum_v2::MessageType::SubmitSharesStandard) == 0x20);
    TEST_RESULT("SetTarget",
        static_cast<uint8_t>(stratum_v2::MessageType::SetTarget) == 0x30);
    TEST_RESULT("SetupConnection",
        static_cast<uint8_t>(stratum_v2::MessageType::SetupConnection) == 0x40);
}

void test_share_submission() {
    TEST_GROUP("Stratum V2 Share Submission");

    stratum_v2::ShareSubmission share;
    share.channel_id = 1;
    share.job_id = 12345;
    share.nonce = 0xDEADBEEF;
    share.ntime = 0x12345678;
    share.version = 0x20000000;

    // Serialize
    stratum_v2::ByteBuffer buf;
    buf.write_u32(share.channel_id);
    buf.write_u64(share.job_id);
    buf.write_u32(share.nonce);
    buf.write_u32(share.ntime);
    buf.write_u32(share.version);

    // Deserialize
    buf.reset();
    stratum_v2::ShareSubmission read_share;
    read_share.channel_id = buf.read_u32();
    read_share.job_id = buf.read_u64();
    read_share.nonce = buf.read_u32();
    read_share.ntime = buf.read_u32();
    read_share.version = buf.read_u32();

    TEST_RESULT("share channel_id", read_share.channel_id == share.channel_id);
    TEST_RESULT("share job_id", read_share.job_id == share.job_id);
    TEST_RESULT("share nonce", read_share.nonce == share.nonce);
    TEST_RESULT("share ntime", read_share.ntime == share.ntime);
    TEST_RESULT("share version", read_share.version == share.version);
}

void test_setup_connection_info() {
    TEST_GROUP("Stratum V2 Setup Connection");

    stratum_v2::SetupConnectionInfo info;
    info.protocol_version = 2;
    info.min_version = 2;
    info.flags = 0;
    info.endpoint_host = "pool.example.com";
    info.endpoint_port = 3333;
    info.vendor = "btcminer";
    info.hw_version = "1.0";
    info.sw_version = "1.0.0";
    info.device_id = "device123";

    // Serialize
    stratum_v2::ByteBuffer buf;
    buf.write_u16(info.protocol_version);
    buf.write_u16(info.min_version);
    buf.write_u32(info.flags);
    buf.write_string(info.endpoint_host);
    buf.write_u16(info.endpoint_port);
    buf.write_string(info.vendor);
    buf.write_string(info.hw_version);
    buf.write_string(info.sw_version);
    buf.write_string(info.device_id);

    // Deserialize
    buf.reset();
    stratum_v2::SetupConnectionInfo read_info;
    read_info.protocol_version = buf.read_u16();
    read_info.min_version = buf.read_u16();
    read_info.flags = buf.read_u32();
    read_info.endpoint_host = buf.read_string();
    read_info.endpoint_port = buf.read_u16();
    read_info.vendor = buf.read_string();
    read_info.hw_version = buf.read_string();
    read_info.sw_version = buf.read_string();
    read_info.device_id = buf.read_string();

    TEST_RESULT("protocol_version", read_info.protocol_version == info.protocol_version);
    TEST_RESULT("endpoint_host", read_info.endpoint_host == info.endpoint_host);
    TEST_RESULT("endpoint_port", read_info.endpoint_port == info.endpoint_port);
    TEST_RESULT("vendor", read_info.vendor == info.vendor);
    TEST_RESULT("device_id", read_info.device_id == info.device_id);
}

void run_all() {
    test_byte_buffer_write();
    test_byte_buffer_read();
    test_byte_buffer_roundtrip();
    test_message_types();
    test_share_submission();
    test_setup_connection_info();
}

} // namespace test_stratum_v2
