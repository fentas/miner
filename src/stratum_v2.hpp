#pragma once

// Stratum V2 Protocol Implementation
// Binary-based mining protocol with Noise encryption

#include <cstdint>
#include <string>
#include <vector>
#include <functional>
#include <atomic>
#include <mutex>
#include <array>
#include <optional>

namespace stratum_v2 {

// Message types (Mining Protocol)
enum class MessageType : uint8_t {
    // Mining Protocol - Channel Messages
    OpenStandardMiningChannel = 0x00,
    OpenStandardMiningChannelSuccess = 0x01,
    OpenStandardMiningChannelError = 0x02,
    UpdateChannel = 0x04,
    UpdateChannelError = 0x05,
    CloseChannel = 0x06,

    // Mining Protocol - Job Messages
    SetNewPrevHash = 0x10,
    NewMiningJob = 0x11,
    SetCustomMiningJob = 0x12,
    SetCustomMiningJobSuccess = 0x13,
    SetCustomMiningJobError = 0x14,

    // Mining Protocol - Share Submission
    SubmitSharesStandard = 0x20,
    SubmitSharesExtended = 0x21,
    SubmitSharesSuccess = 0x22,
    SubmitSharesError = 0x23,

    // Mining Protocol - Difficulty
    SetTarget = 0x30,

    // Common Messages
    SetupConnection = 0x40,
    SetupConnectionSuccess = 0x41,
    SetupConnectionError = 0x42,

    // Reconnection
    Reconnect = 0x50,
};

// Protocol version
constexpr uint16_t PROTOCOL_VERSION = 2;

// Maximum message size
constexpr size_t MAX_MESSAGE_SIZE = 65535;

// Channel ID type
using ChannelId = uint32_t;

// Job ID type (V2 uses a more structured job system)
struct JobId {
    uint64_t id;
    bool is_future;  // If true, job is for next block
};

// Mining job from pool
struct MiningJob {
    JobId job_id;
    uint32_t channel_id;
    std::array<uint8_t, 32> prev_hash;
    uint32_t min_nonce;
    uint32_t max_nonce;
    uint32_t version;
    uint32_t version_rolling_mask;
    bool merkle_path_provided;
    std::vector<std::array<uint8_t, 32>> merkle_path;
    std::array<uint8_t, 32> coinbase_tx_prefix;
    std::array<uint8_t, 32> coinbase_tx_suffix;
};

// Target (difficulty)
struct Target {
    uint32_t max_target_bits;  // Compact target representation
};

// Share submission
struct ShareSubmission {
    uint32_t channel_id;
    uint64_t job_id;
    uint32_t nonce;
    uint32_t ntime;
    uint32_t version;
};

// Connection setup info
struct SetupConnectionInfo {
    uint16_t protocol_version;
    uint16_t min_version;
    uint32_t flags;
    std::string endpoint_host;
    uint16_t endpoint_port;
    std::string vendor;
    std::string hw_version;
    std::string sw_version;
    std::string device_id;
};

// Binary message header
struct MessageHeader {
    uint16_t extension_type;  // Extension type (0 for base protocol)
    MessageType msg_type;
    uint32_t msg_length;
};

// Simple byte buffer for message serialization
class ByteBuffer {
public:
    ByteBuffer() = default;
    explicit ByteBuffer(size_t capacity) { data_.reserve(capacity); }
    explicit ByteBuffer(const std::vector<uint8_t>& data) : data_(data) {}

    // Write methods
    void write_u8(uint8_t v);
    void write_u16(uint16_t v);
    void write_u24(uint32_t v);
    void write_u32(uint32_t v);
    void write_u64(uint64_t v);
    void write_bytes(const uint8_t* data, size_t len);
    void write_string(const std::string& s);
    void write_bool(bool v);

    // Read methods
    uint8_t read_u8();
    uint16_t read_u16();
    uint32_t read_u24();
    uint32_t read_u32();
    uint64_t read_u64();
    void read_bytes(uint8_t* data, size_t len);
    std::string read_string();
    bool read_bool();

    // Access
    const uint8_t* data() const { return data_.data(); }
    size_t size() const { return data_.size(); }
    size_t remaining() const { return data_.size() - pos_; }
    void reset() { pos_ = 0; }
    void clear() { data_.clear(); pos_ = 0; }

    std::vector<uint8_t>& buffer() { return data_; }
    const std::vector<uint8_t>& buffer() const { return data_; }

private:
    std::vector<uint8_t> data_;
    size_t pos_ = 0;
};

// Noise Protocol encryption (simplified)
// Full implementation would use libsodium or similar
class NoiseSession {
public:
    NoiseSession() = default;

    // Initialize as initiator (client) or responder (server)
    bool init_initiator(const std::array<uint8_t, 32>& server_public_key);
    bool init_responder(const std::array<uint8_t, 32>& private_key,
                        const std::array<uint8_t, 32>& public_key);

    // Handshake messages
    std::vector<uint8_t> handshake_write();
    bool handshake_read(const std::vector<uint8_t>& msg);
    bool is_handshake_complete() const { return handshake_complete_; }

    // Encrypt/decrypt after handshake
    std::vector<uint8_t> encrypt(const std::vector<uint8_t>& plaintext);
    std::vector<uint8_t> decrypt(const std::vector<uint8_t>& ciphertext);

private:
    bool handshake_complete_ = false;
    std::array<uint8_t, 32> send_key_;
    std::array<uint8_t, 32> recv_key_;
    uint64_t send_nonce_ = 0;
    uint64_t recv_nonce_ = 0;
};

// Callback types
using JobCallback = std::function<void(const MiningJob& job)>;
using TargetCallback = std::function<void(const Target& target)>;
using ShareResultCallback = std::function<void(uint64_t job_id, bool accepted, const std::string& error)>;
using ConnectionCallback = std::function<void(bool connected, const std::string& message)>;

// Stratum V2 Client
class StratumV2Client {
public:
    StratumV2Client();
    ~StratumV2Client();

    // Connection
    bool connect(const std::string& host, uint16_t port,
                 const std::string& user,
                 const std::optional<std::array<uint8_t, 32>>& server_pubkey = std::nullopt);
    void disconnect();
    bool is_connected() const { return connected_.load(); }

    // Callbacks
    void set_job_callback(JobCallback cb) { job_callback_ = cb; }
    void set_target_callback(TargetCallback cb) { target_callback_ = cb; }
    void set_share_result_callback(ShareResultCallback cb) { share_result_callback_ = cb; }
    void set_connection_callback(ConnectionCallback cb) { connection_callback_ = cb; }

    // Mining operations
    bool open_channel(float nominal_hashrate);
    bool submit_share(const ShareSubmission& share);

    // Run receive loop (blocking)
    void run();

    // Request stop
    void stop() { running_ = false; }

    // Get channel info
    ChannelId get_channel_id() const { return channel_id_; }
    std::string get_extranonce_prefix() const { return extranonce_prefix_; }

    // Stats
    uint64_t get_shares_accepted() const { return shares_accepted_; }
    uint64_t get_shares_rejected() const { return shares_rejected_; }
    double get_difficulty() const { return difficulty_; }

private:
    // Socket operations
    bool send_message(MessageType type, const ByteBuffer& payload);
    bool receive_message(MessageHeader& header, ByteBuffer& payload);

    // Message handlers
    void handle_setup_connection_success(ByteBuffer& payload);
    void handle_setup_connection_error(ByteBuffer& payload);
    void handle_open_channel_success(ByteBuffer& payload);
    void handle_open_channel_error(ByteBuffer& payload);
    void handle_new_mining_job(ByteBuffer& payload);
    void handle_set_new_prev_hash(ByteBuffer& payload);
    void handle_set_target(ByteBuffer& payload);
    void handle_submit_shares_success(ByteBuffer& payload);
    void handle_submit_shares_error(ByteBuffer& payload);

    // Build messages
    ByteBuffer build_setup_connection(const SetupConnectionInfo& info);
    ByteBuffer build_open_channel(uint32_t req_id, const std::string& user,
                                   float nominal_hashrate, uint32_t max_target);

    // Connection state
    int socket_fd_ = -1;
    std::atomic<bool> connected_{false};
    std::atomic<bool> running_{false};
    bool use_encryption_ = false;
    NoiseSession noise_;

    // Channel state
    ChannelId channel_id_ = 0;
    std::string extranonce_prefix_;
    uint32_t extranonce2_size_ = 0;

    // Current job
    MiningJob current_job_;
    std::array<uint8_t, 32> current_prev_hash_;
    std::mutex job_mutex_;

    // User info
    std::string user_;
    std::string host_;
    uint16_t port_ = 0;

    // Stats
    std::atomic<uint64_t> shares_accepted_{0};
    std::atomic<uint64_t> shares_rejected_{0};
    std::atomic<double> difficulty_{1.0};

    // Callbacks
    JobCallback job_callback_;
    TargetCallback target_callback_;
    ShareResultCallback share_result_callback_;
    ConnectionCallback connection_callback_;

    // Pending requests
    std::atomic<uint32_t> next_request_id_{1};
};

} // namespace stratum_v2
