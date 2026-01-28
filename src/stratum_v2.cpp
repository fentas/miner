// Stratum V2 Protocol Implementation

#include "stratum_v2.hpp"

#include <iostream>
#include <cstring>
#include <cmath>
#include <unistd.h>
#include <sys/socket.h>
#include <netinet/in.h>
#include <netdb.h>
#include <fcntl.h>
#include <poll.h>

namespace stratum_v2 {

// ByteBuffer implementation

void ByteBuffer::write_u8(uint8_t v) {
    data_.push_back(v);
}

void ByteBuffer::write_u16(uint16_t v) {
    // Little-endian
    data_.push_back(v & 0xff);
    data_.push_back((v >> 8) & 0xff);
}

void ByteBuffer::write_u24(uint32_t v) {
    data_.push_back(v & 0xff);
    data_.push_back((v >> 8) & 0xff);
    data_.push_back((v >> 16) & 0xff);
}

void ByteBuffer::write_u32(uint32_t v) {
    data_.push_back(v & 0xff);
    data_.push_back((v >> 8) & 0xff);
    data_.push_back((v >> 16) & 0xff);
    data_.push_back((v >> 24) & 0xff);
}

void ByteBuffer::write_u64(uint64_t v) {
    for (int i = 0; i < 8; i++) {
        data_.push_back((v >> (i * 8)) & 0xff);
    }
}

void ByteBuffer::write_bytes(const uint8_t* data, size_t len) {
    data_.insert(data_.end(), data, data + len);
}

void ByteBuffer::write_string(const std::string& s) {
    // String format: u8 length + data (short strings up to 255 bytes)
    if (s.size() > 255) {
        write_u8(255);
        write_bytes(reinterpret_cast<const uint8_t*>(s.data()), 255);
    } else {
        write_u8(static_cast<uint8_t>(s.size()));
        write_bytes(reinterpret_cast<const uint8_t*>(s.data()), s.size());
    }
}

void ByteBuffer::write_bool(bool v) {
    write_u8(v ? 1 : 0);
}

uint8_t ByteBuffer::read_u8() {
    if (pos_ >= data_.size()) return 0;
    return data_[pos_++];
}

uint16_t ByteBuffer::read_u16() {
    uint16_t v = 0;
    v |= read_u8();
    v |= static_cast<uint16_t>(read_u8()) << 8;
    return v;
}

uint32_t ByteBuffer::read_u24() {
    uint32_t v = 0;
    v |= read_u8();
    v |= static_cast<uint32_t>(read_u8()) << 8;
    v |= static_cast<uint32_t>(read_u8()) << 16;
    return v;
}

uint32_t ByteBuffer::read_u32() {
    uint32_t v = 0;
    v |= read_u8();
    v |= static_cast<uint32_t>(read_u8()) << 8;
    v |= static_cast<uint32_t>(read_u8()) << 16;
    v |= static_cast<uint32_t>(read_u8()) << 24;
    return v;
}

uint64_t ByteBuffer::read_u64() {
    uint64_t v = 0;
    for (int i = 0; i < 8; i++) {
        v |= static_cast<uint64_t>(read_u8()) << (i * 8);
    }
    return v;
}

void ByteBuffer::read_bytes(uint8_t* data, size_t len) {
    for (size_t i = 0; i < len && pos_ < data_.size(); i++) {
        data[i] = data_[pos_++];
    }
}

std::string ByteBuffer::read_string() {
    uint8_t len = read_u8();
    std::string s;
    s.reserve(len);
    for (uint8_t i = 0; i < len && pos_ < data_.size(); i++) {
        s.push_back(static_cast<char>(data_[pos_++]));
    }
    return s;
}

bool ByteBuffer::read_bool() {
    return read_u8() != 0;
}

// NoiseSession implementation (simplified placeholder)
// A full implementation would use a proper Noise library

bool NoiseSession::init_initiator(const std::array<uint8_t, 32>& server_public_key) {
    // In a real implementation, this would:
    // 1. Generate ephemeral keypair
    // 2. Perform Noise_NX handshake
    // For now, just copy the key (insecure placeholder)
    send_key_ = server_public_key;
    recv_key_ = server_public_key;
    return true;
}

bool NoiseSession::init_responder(const std::array<uint8_t, 32>& private_key,
                                   const std::array<uint8_t, 32>& public_key) {
    send_key_ = private_key;
    recv_key_ = public_key;
    return true;
}

std::vector<uint8_t> NoiseSession::handshake_write() {
    // Placeholder: return empty handshake
    handshake_complete_ = true;
    return {};
}

bool NoiseSession::handshake_read(const std::vector<uint8_t>& msg) {
    (void)msg;
    handshake_complete_ = true;
    return true;
}

std::vector<uint8_t> NoiseSession::encrypt(const std::vector<uint8_t>& plaintext) {
    // Placeholder: no actual encryption
    // Real implementation would use ChaCha20-Poly1305
    send_nonce_++;
    return plaintext;
}

std::vector<uint8_t> NoiseSession::decrypt(const std::vector<uint8_t>& ciphertext) {
    // Placeholder: no actual decryption
    recv_nonce_++;
    return ciphertext;
}

// StratumV2Client implementation

StratumV2Client::StratumV2Client() = default;

StratumV2Client::~StratumV2Client() {
    disconnect();
}

bool StratumV2Client::connect(const std::string& host, uint16_t port,
                               const std::string& user,
                               const std::optional<std::array<uint8_t, 32>>& server_pubkey) {
    host_ = host;
    port_ = port;
    user_ = user;
    use_encryption_ = server_pubkey.has_value();

    // Resolve hostname
    struct hostent* he = gethostbyname(host.c_str());
    if (!he) {
        std::cerr << "[StratumV2] Failed to resolve hostname: " << host << std::endl;
        return false;
    }

    // Create socket
    socket_fd_ = socket(AF_INET, SOCK_STREAM, 0);
    if (socket_fd_ < 0) {
        std::cerr << "[StratumV2] Failed to create socket" << std::endl;
        return false;
    }

    // Connect
    struct sockaddr_in addr;
    std::memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port);
    std::memcpy(&addr.sin_addr, he->h_addr_list[0], he->h_length);

    if (::connect(socket_fd_, reinterpret_cast<struct sockaddr*>(&addr), sizeof(addr)) < 0) {
        std::cerr << "[StratumV2] Failed to connect to " << host << ":" << port << std::endl;
        close(socket_fd_);
        socket_fd_ = -1;
        return false;
    }

    std::cout << "[StratumV2] Connected to " << host << ":" << port << std::endl;

    // Perform Noise handshake if encryption enabled
    if (use_encryption_ && server_pubkey) {
        if (!noise_.init_initiator(*server_pubkey)) {
            std::cerr << "[StratumV2] Failed to initialize encryption" << std::endl;
            disconnect();
            return false;
        }

        auto handshake_msg = noise_.handshake_write();
        if (!handshake_msg.empty()) {
            if (send(socket_fd_, handshake_msg.data(), handshake_msg.size(), 0) < 0) {
                std::cerr << "[StratumV2] Failed to send handshake" << std::endl;
                disconnect();
                return false;
            }
        }
    }

    // Send SetupConnection
    SetupConnectionInfo setup_info;
    setup_info.protocol_version = PROTOCOL_VERSION;
    setup_info.min_version = PROTOCOL_VERSION;
    setup_info.flags = 0;
    setup_info.endpoint_host = host;
    setup_info.endpoint_port = port;
    setup_info.vendor = "btcminer";
    setup_info.hw_version = "1.0";
    setup_info.sw_version = "1.0.0";
    setup_info.device_id = "";

    auto setup_msg = build_setup_connection(setup_info);
    if (!send_message(MessageType::SetupConnection, setup_msg)) {
        std::cerr << "[StratumV2] Failed to send SetupConnection" << std::endl;
        disconnect();
        return false;
    }

    // Wait for response
    MessageHeader header;
    ByteBuffer response;
    if (!receive_message(header, response)) {
        std::cerr << "[StratumV2] Failed to receive SetupConnection response" << std::endl;
        disconnect();
        return false;
    }

    if (header.msg_type == MessageType::SetupConnectionSuccess) {
        handle_setup_connection_success(response);
        connected_ = true;
        running_ = true;

        if (connection_callback_) {
            connection_callback_(true, "Connected");
        }
        return true;
    } else if (header.msg_type == MessageType::SetupConnectionError) {
        handle_setup_connection_error(response);
        disconnect();
        return false;
    }

    std::cerr << "[StratumV2] Unexpected response type" << std::endl;
    disconnect();
    return false;
}

void StratumV2Client::disconnect() {
    running_ = false;
    connected_ = false;

    if (socket_fd_ >= 0) {
        close(socket_fd_);
        socket_fd_ = -1;
    }

    if (connection_callback_) {
        connection_callback_(false, "Disconnected");
    }
}

bool StratumV2Client::open_channel(float nominal_hashrate) {
    if (!connected_) return false;

    uint32_t req_id = next_request_id_++;
    uint32_t max_target = 0xFFFF0000;  // Easy initial target

    auto msg = build_open_channel(req_id, user_, nominal_hashrate, max_target);
    if (!send_message(MessageType::OpenStandardMiningChannel, msg)) {
        return false;
    }

    // Wait for response
    MessageHeader header;
    ByteBuffer response;
    if (!receive_message(header, response)) {
        return false;
    }

    if (header.msg_type == MessageType::OpenStandardMiningChannelSuccess) {
        handle_open_channel_success(response);
        return true;
    } else if (header.msg_type == MessageType::OpenStandardMiningChannelError) {
        handle_open_channel_error(response);
        return false;
    }

    return false;
}

bool StratumV2Client::submit_share(const ShareSubmission& share) {
    if (!connected_) return false;

    ByteBuffer payload;
    payload.write_u32(share.channel_id);
    payload.write_u64(share.job_id);
    payload.write_u32(share.nonce);
    payload.write_u32(share.ntime);
    payload.write_u32(share.version);

    return send_message(MessageType::SubmitSharesStandard, payload);
}

void StratumV2Client::run() {
    while (running_ && socket_fd_ >= 0) {
        // Poll for data
        struct pollfd pfd;
        pfd.fd = socket_fd_;
        pfd.events = POLLIN;

        int ret = poll(&pfd, 1, 1000);  // 1 second timeout
        if (ret < 0) {
            std::cerr << "[StratumV2] Poll error" << std::endl;
            break;
        }

        if (ret == 0) continue;  // Timeout

        if (pfd.revents & POLLIN) {
            MessageHeader header;
            ByteBuffer payload;

            if (!receive_message(header, payload)) {
                std::cerr << "[StratumV2] Failed to receive message" << std::endl;
                break;
            }

            // Handle message
            switch (header.msg_type) {
                case MessageType::NewMiningJob:
                    handle_new_mining_job(payload);
                    break;
                case MessageType::SetNewPrevHash:
                    handle_set_new_prev_hash(payload);
                    break;
                case MessageType::SetTarget:
                    handle_set_target(payload);
                    break;
                case MessageType::SubmitSharesSuccess:
                    handle_submit_shares_success(payload);
                    break;
                case MessageType::SubmitSharesError:
                    handle_submit_shares_error(payload);
                    break;
                default:
                    std::cout << "[StratumV2] Unknown message type: "
                              << static_cast<int>(header.msg_type) << std::endl;
                    break;
            }
        }

        if (pfd.revents & (POLLERR | POLLHUP)) {
            std::cerr << "[StratumV2] Connection lost" << std::endl;
            break;
        }
    }

    connected_ = false;
}

bool StratumV2Client::send_message(MessageType type, const ByteBuffer& payload) {
    // Build header
    std::vector<uint8_t> msg;
    msg.reserve(6 + payload.size());

    // Extension type (2 bytes) - always 0 for base protocol
    msg.push_back(0);
    msg.push_back(0);

    // Message type (1 byte)
    msg.push_back(static_cast<uint8_t>(type));

    // Message length (3 bytes, little-endian)
    uint32_t len = static_cast<uint32_t>(payload.size());
    msg.push_back(len & 0xff);
    msg.push_back((len >> 8) & 0xff);
    msg.push_back((len >> 16) & 0xff);

    // Payload
    msg.insert(msg.end(), payload.data(), payload.data() + payload.size());

    // Encrypt if needed
    if (use_encryption_ && noise_.is_handshake_complete()) {
        msg = noise_.encrypt(msg);
    }

    // Send
    ssize_t sent = send(socket_fd_, msg.data(), msg.size(), 0);
    return sent == static_cast<ssize_t>(msg.size());
}

bool StratumV2Client::receive_message(MessageHeader& header, ByteBuffer& payload) {
    // Read header (6 bytes)
    uint8_t header_buf[6];
    ssize_t received = recv(socket_fd_, header_buf, 6, MSG_WAITALL);
    if (received != 6) {
        return false;
    }

    // Decrypt if needed
    if (use_encryption_ && noise_.is_handshake_complete()) {
        std::vector<uint8_t> encrypted(header_buf, header_buf + 6);
        auto decrypted = noise_.decrypt(encrypted);
        if (decrypted.size() != 6) return false;
        std::memcpy(header_buf, decrypted.data(), 6);
    }

    // Parse header
    header.extension_type = header_buf[0] | (static_cast<uint16_t>(header_buf[1]) << 8);
    header.msg_type = static_cast<MessageType>(header_buf[2]);
    header.msg_length = header_buf[3] |
                        (static_cast<uint32_t>(header_buf[4]) << 8) |
                        (static_cast<uint32_t>(header_buf[5]) << 16);

    if (header.msg_length > MAX_MESSAGE_SIZE) {
        return false;
    }

    // Read payload
    if (header.msg_length > 0) {
        std::vector<uint8_t> buf(header.msg_length);
        received = recv(socket_fd_, buf.data(), header.msg_length, MSG_WAITALL);
        if (received != static_cast<ssize_t>(header.msg_length)) {
            return false;
        }

        if (use_encryption_ && noise_.is_handshake_complete()) {
            buf = noise_.decrypt(buf);
        }

        payload = ByteBuffer(buf);
    }

    return true;
}

void StratumV2Client::handle_setup_connection_success(ByteBuffer& payload) {
    uint16_t used_version = payload.read_u16();
    uint32_t flags = payload.read_u32();

    std::cout << "[StratumV2] Setup connection success, version: " << used_version
              << ", flags: 0x" << std::hex << flags << std::dec << std::endl;
}

void StratumV2Client::handle_setup_connection_error(ByteBuffer& payload) {
    uint32_t flags = payload.read_u32();
    std::string error_code = payload.read_string();

    std::cerr << "[StratumV2] Setup connection error: " << error_code
              << " (flags: 0x" << std::hex << flags << std::dec << ")" << std::endl;
}

void StratumV2Client::handle_open_channel_success(ByteBuffer& payload) {
    uint32_t request_id = payload.read_u32();
    channel_id_ = payload.read_u32();

    // Read target (32 bytes)
    uint8_t target[32];
    payload.read_bytes(target, 32);

    // Read extranonce prefix
    uint8_t prefix_len = payload.read_u8();
    std::vector<uint8_t> prefix(prefix_len);
    payload.read_bytes(prefix.data(), prefix_len);

    extranonce_prefix_.clear();
    for (uint8_t b : prefix) {
        char hex[3];
        snprintf(hex, sizeof(hex), "%02x", b);
        extranonce_prefix_ += hex;
    }

    extranonce2_size_ = payload.read_u32();

    std::cout << "[StratumV2] Channel opened: id=" << channel_id_
              << ", extranonce_prefix=" << extranonce_prefix_
              << ", extranonce2_size=" << extranonce2_size_ << std::endl;

    (void)request_id;  // Unused for now
}

void StratumV2Client::handle_open_channel_error(ByteBuffer& payload) {
    uint32_t request_id = payload.read_u32();
    std::string error_code = payload.read_string();

    std::cerr << "[StratumV2] Open channel error: " << error_code << std::endl;
    (void)request_id;
}

void StratumV2Client::handle_new_mining_job(ByteBuffer& payload) {
    std::lock_guard<std::mutex> lock(job_mutex_);

    current_job_.channel_id = payload.read_u32();
    current_job_.job_id.id = payload.read_u64();
    current_job_.job_id.is_future = payload.read_bool();
    current_job_.version = payload.read_u32();
    current_job_.version_rolling_mask = payload.read_u32();

    // Read merkle path
    uint8_t merkle_count = payload.read_u8();
    current_job_.merkle_path.resize(merkle_count);
    for (uint8_t i = 0; i < merkle_count; i++) {
        payload.read_bytes(current_job_.merkle_path[i].data(), 32);
    }

    // Read coinbase prefix/suffix
    uint16_t prefix_len = payload.read_u16();
    if (prefix_len <= 32) {
        payload.read_bytes(current_job_.coinbase_tx_prefix.data(), prefix_len);
    }

    uint16_t suffix_len = payload.read_u16();
    if (suffix_len <= 32) {
        payload.read_bytes(current_job_.coinbase_tx_suffix.data(), suffix_len);
    }

    std::cout << "[StratumV2] New job: id=" << current_job_.job_id.id
              << (current_job_.job_id.is_future ? " (future)" : "") << std::endl;

    if (job_callback_) {
        job_callback_(current_job_);
    }
}

void StratumV2Client::handle_set_new_prev_hash(ByteBuffer& payload) {
    std::lock_guard<std::mutex> lock(job_mutex_);

    uint32_t channel_id = payload.read_u32();
    uint64_t job_id = payload.read_u64();
    payload.read_bytes(current_prev_hash_.data(), 32);
    uint32_t min_nonce = payload.read_u32();
    uint32_t nbits = payload.read_u32();

    current_job_.prev_hash = current_prev_hash_;
    current_job_.min_nonce = min_nonce;

    std::cout << "[StratumV2] New prev_hash for job " << job_id << std::endl;

    (void)channel_id;
    (void)nbits;
}

void StratumV2Client::handle_set_target(ByteBuffer& payload) {
    uint32_t channel_id = payload.read_u32();
    uint32_t max_target = payload.read_u32();

    // Convert compact target to difficulty
    uint32_t mantissa = max_target & 0x007fffff;
    uint32_t exponent = (max_target >> 24) & 0xff;

    if (exponent <= 3) {
        difficulty_ = static_cast<double>(mantissa >> (8 * (3 - exponent)));
    } else {
        difficulty_ = static_cast<double>(mantissa) * (1ULL << (8 * (exponent - 3)));
    }

    // Convert to pool difficulty (simplified)
    // Full formula: difficulty = 0xFFFF * 2^208 / target
    // We use a simpler approximation here
    if (difficulty_ > 0) {
        difficulty_ = 0xFFFFULL * std::pow(2.0, 208) / difficulty_;
    }

    std::cout << "[StratumV2] Target set, difficulty: " << difficulty_.load() << std::endl;

    if (target_callback_) {
        Target target;
        target.max_target_bits = max_target;
        target_callback_(target);
    }

    (void)channel_id;
}

void StratumV2Client::handle_submit_shares_success(ByteBuffer& payload) {
    uint32_t channel_id = payload.read_u32();
    uint64_t job_id = payload.read_u64();
    uint32_t sequence_num = payload.read_u32();

    shares_accepted_++;

    std::cout << "[StratumV2] Share accepted: job=" << job_id
              << ", seq=" << sequence_num << std::endl;

    if (share_result_callback_) {
        share_result_callback_(job_id, true, "");
    }

    (void)channel_id;
}

void StratumV2Client::handle_submit_shares_error(ByteBuffer& payload) {
    uint32_t channel_id = payload.read_u32();
    uint64_t job_id = payload.read_u64();
    uint32_t sequence_num = payload.read_u32();
    std::string error_code = payload.read_string();

    shares_rejected_++;

    std::cerr << "[StratumV2] Share rejected: job=" << job_id
              << ", error=" << error_code << std::endl;

    if (share_result_callback_) {
        share_result_callback_(job_id, false, error_code);
    }

    (void)channel_id;
    (void)sequence_num;
}

ByteBuffer StratumV2Client::build_setup_connection(const SetupConnectionInfo& info) {
    ByteBuffer buf;

    buf.write_u16(info.protocol_version);
    buf.write_u16(info.min_version);
    buf.write_u32(info.flags);
    buf.write_string(info.endpoint_host);
    buf.write_u16(info.endpoint_port);
    buf.write_string(info.vendor);
    buf.write_string(info.hw_version);
    buf.write_string(info.sw_version);
    buf.write_string(info.device_id);

    return buf;
}

ByteBuffer StratumV2Client::build_open_channel(uint32_t req_id, const std::string& user,
                                                float nominal_hashrate, uint32_t max_target) {
    ByteBuffer buf;

    buf.write_u32(req_id);
    buf.write_string(user);
    buf.write_u32(*reinterpret_cast<uint32_t*>(&nominal_hashrate));  // IEEE 754 float
    buf.write_u32(max_target);

    return buf;
}

} // namespace stratum_v2
