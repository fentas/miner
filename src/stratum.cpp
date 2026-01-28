#include "stratum.hpp"
#include "sha256.hpp"

#include <sys/socket.h>
#include <netinet/in.h>
#include <arpa/inet.h>
#include <netdb.h>
#include <unistd.h>
#include <cstring>
#include <sstream>
#include <iostream>
#include <iomanip>
#include <algorithm>

// Simple JSON parsing (minimal, no external dependencies)
namespace {

[[maybe_unused]]
std::string json_get_string(const std::string& json, const std::string& key) {
    std::string search = "\"" + key + "\"";
    size_t pos = json.find(search);
    if (pos == std::string::npos) return "";

    pos = json.find(':', pos);
    if (pos == std::string::npos) return "";

    pos = json.find('"', pos);
    if (pos == std::string::npos) return "";

    size_t end = json.find('"', pos + 1);
    if (end == std::string::npos) return "";

    return json.substr(pos + 1, end - pos - 1);
}

[[maybe_unused]]
bool json_get_bool(const std::string& json, const std::string& key) {
    std::string search = "\"" + key + "\"";
    size_t pos = json.find(search);
    if (pos == std::string::npos) return false;

    pos = json.find(':', pos);
    if (pos == std::string::npos) return false;

    return json.find("true", pos) < json.find(',', pos);
}

[[maybe_unused]]
std::vector<std::string> json_get_array(const std::string& json, size_t start_pos = 0) {
    std::vector<std::string> result;
    size_t pos = json.find('[', start_pos);
    if (pos == std::string::npos) return result;

    size_t end = json.find(']', pos);
    if (end == std::string::npos) return result;

    std::string arr = json.substr(pos + 1, end - pos - 1);

    // Parse array elements
    size_t i = 0;
    while (i < arr.size()) {
        while (i < arr.size() && (arr[i] == ' ' || arr[i] == ',')) i++;
        if (i >= arr.size()) break;

        if (arr[i] == '"') {
            size_t str_end = arr.find('"', i + 1);
            if (str_end != std::string::npos) {
                result.push_back(arr.substr(i + 1, str_end - i - 1));
                i = str_end + 1;
            } else break;
        } else if (arr[i] == '[') {
            // Nested array - skip for now
            size_t nest_end = arr.find(']', i);
            if (nest_end != std::string::npos) i = nest_end + 1;
            else break;
        } else {
            size_t val_end = arr.find_first_of(",]", i);
            if (val_end == std::string::npos) val_end = arr.size();
            std::string val = arr.substr(i, val_end - i);
            // Trim whitespace
            val.erase(0, val.find_first_not_of(" \t"));
            val.erase(val.find_last_not_of(" \t") + 1);
            if (!val.empty()) result.push_back(val);
            i = val_end;
        }
    }
    return result;
}

// Extract params array from mining.notify
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

} // anonymous namespace

namespace stratum {

Client::Client(const std::string& host, uint16_t port,
               const std::string& user, const std::string& password)
    : host_(host), port_(port), user_(user), password_(password) {}

Client::~Client() {
    disconnect();
}

bool Client::connect() {
    struct hostent* he = gethostbyname(host_.c_str());
    if (!he) {
        std::cerr << "[Stratum] Failed to resolve host: " << host_ << std::endl;
        return false;
    }

    sock_ = socket(AF_INET, SOCK_STREAM, 0);
    if (sock_ < 0) {
        std::cerr << "[Stratum] Failed to create socket" << std::endl;
        return false;
    }

    struct sockaddr_in addr;
    std::memset(&addr, 0, sizeof(addr));
    addr.sin_family = AF_INET;
    addr.sin_port = htons(port_);
    std::memcpy(&addr.sin_addr, he->h_addr_list[0], he->h_length);

    if (::connect(sock_, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
        std::cerr << "[Stratum] Failed to connect to " << host_ << ":" << port_ << std::endl;
        close(sock_);
        sock_ = -1;
        return false;
    }

    std::cout << "[Stratum] Connected to " << host_ << ":" << port_ << std::endl;

    if (!subscribe()) {
        disconnect();
        return false;
    }

    if (!authorize()) {
        disconnect();
        return false;
    }

    connected_ = true;
    return true;
}

void Client::disconnect() {
    running_ = false;
    connected_ = false;
    if (sock_ >= 0) {
        close(sock_);
        sock_ = -1;
    }
}

bool Client::send_json(const std::string& json) {
    std::lock_guard<std::mutex> lock(send_mutex_);
    std::string msg = json + "\n";
    ssize_t sent = send(sock_, msg.c_str(), msg.size(), 0);
    return sent == static_cast<ssize_t>(msg.size());
}

std::string Client::recv_line() {
    std::string line;
    char c;
    while (recv(sock_, &c, 1, 0) == 1) {
        if (c == '\n') break;
        line += c;
    }
    return line;
}

bool Client::subscribe() {
    std::ostringstream oss;
    oss << "{\"id\": " << msg_id_++ << ", \"method\": \"mining.subscribe\", \"params\": []}";

    if (!send_json(oss.str())) {
        std::cerr << "[Stratum] Failed to send subscribe" << std::endl;
        return false;
    }

    std::string response = recv_line();
    std::cout << "[Stratum] Subscribe response: " << response << std::endl;

    // Parse extranonce1 and extranonce2_size from result
    // Result format: [[["mining.set_difficulty", "..."], ["mining.notify", "..."]], "extranonce1", extranonce2_size]
    size_t result_pos = response.find("\"result\"");
    if (result_pos == std::string::npos) {
        std::cerr << "[Stratum] No result in subscribe response" << std::endl;
        return false;
    }

    // Find extranonce1 (first quoted string after the nested arrays)
    size_t arr_end = response.rfind("]]");
    if (arr_end == std::string::npos) return false;

    size_t en1_start = response.find('"', arr_end + 2);
    size_t en1_end = response.find('"', en1_start + 1);
    if (en1_start == std::string::npos || en1_end == std::string::npos) return false;

    extranonce1_ = response.substr(en1_start + 1, en1_end - en1_start - 1);

    // Find extranonce2_size (number after extranonce1)
    size_t size_start = response.find(',', en1_end);
    if (size_start != std::string::npos) {
        size_start++;
        while (size_start < response.size() && response[size_start] == ' ') size_start++;
        extranonce2_size_ = std::stoul(response.substr(size_start));
    }

    std::cout << "[Stratum] Extranonce1: " << extranonce1_
              << ", Extranonce2 size: " << extranonce2_size_ << std::endl;

    return true;
}

bool Client::authorize() {
    std::ostringstream oss;
    oss << "{\"id\": " << msg_id_++ << ", \"method\": \"mining.authorize\", "
        << "\"params\": [\"" << user_ << "\", \"" << password_ << "\"]}";

    if (!send_json(oss.str())) {
        std::cerr << "[Stratum] Failed to send authorize" << std::endl;
        return false;
    }

    // Read responses until we get the auth result and first job
    for (int i = 0; i < 5; i++) {
        std::string response = recv_line();
        if (response.empty()) continue;

        std::cout << "[Stratum] Auth response: " << response << std::endl;

        if (response.find("\"result\":true") != std::string::npos ||
            response.find("\"result\": true") != std::string::npos) {
            std::cout << "[Stratum] Authorization successful" << std::endl;
        }

        if (response.find("mining.notify") != std::string::npos) {
            process_message(response);
            return true;
        }
    }

    return true;
}

void Client::run() {
    running_ = true;
    while (running_ && connected_) {
        std::string msg = recv_line();
        if (msg.empty()) {
            if (running_) {
                std::cerr << "[Stratum] Connection lost" << std::endl;
                connected_ = false;
            }
            break;
        }
        process_message(msg);
    }
}

void Client::process_message(const std::string& msg) {
    if (msg.find("mining.notify") != std::string::npos) {
        // Parse mining.notify params
        // Format: {"id":null,"method":"mining.notify","params":["job_id","prevhash","coinb1","coinb2",["merkle"],"version","nbits","ntime",clean_jobs]}

        std::vector<std::string> params = extract_notify_params(msg);

        if (params.size() >= 7) {
            process_notify(params);
        }
    } else if (msg.find("mining.set_difficulty") != std::string::npos) {
        std::cout << "[Stratum] Difficulty update: " << msg << std::endl;
    }
}

void Client::process_notify(const std::vector<std::string>& params) {
    Job job;
    job.job_id = params[0];
    job.prevhash = params[1];
    job.coinb1 = params[2];
    job.coinb2 = params[3];
    // Note: merkle branch array is skipped during parsing, so indices are shifted
    // params[4] = version (merkle branch was at original index 4 but skipped)
    job.version = params[4];
    job.nbits = params[5];
    job.ntime = params[6];
    job.clean_jobs = (params.size() > 7 && params[7] == "true");

    // TODO: Parse merkle branch from the original message

    std::cout << "[Stratum] New job: " << job.job_id
              << " height hint from nbits" << std::endl;

    // Generate a random extranonce2
    std::ostringstream en2;
    for (size_t i = 0; i < extranonce2_size_; i++) {
        en2 << std::hex << std::setw(2) << std::setfill('0') << (rand() & 0xff);
    }

    build_header(job, en2.str());
    calculate_target(job);

    if (job_callback_) {
        job_callback_(job);
    }
}

std::vector<uint8_t> Client::hex_to_bytes(const std::string& hex) {
    std::vector<uint8_t> bytes;
    for (size_t i = 0; i + 1 < hex.length(); i += 2) {
        try {
            bytes.push_back(static_cast<uint8_t>(std::stoul(hex.substr(i, 2), nullptr, 16)));
        } catch (const std::exception&) {
            // Invalid hex character, skip
            break;
        }
    }
    return bytes;
}

std::string Client::bytes_to_hex(const uint8_t* data, size_t len) {
    std::ostringstream oss;
    for (size_t i = 0; i < len; i++) {
        oss << std::hex << std::setw(2) << std::setfill('0') << static_cast<int>(data[i]);
    }
    return oss.str();
}

void Client::build_header(Job& job, const std::string& extranonce2) {
    std::memset(job.header_template, 0, 80);

    // Version (4 bytes, little-endian)
    auto version_bytes = hex_to_bytes(job.version);
    for (int i = 0; i < 4 && i < static_cast<int>(version_bytes.size()); i++) {
        job.header_template[3-i] = version_bytes[i];
    }

    // Previous block hash (32 bytes) - already in internal byte order from pool
    auto prevhash_bytes = hex_to_bytes(job.prevhash);
    // Swap endianness for each 4-byte chunk
    for (int i = 0; i < 8; i++) {
        for (int j = 0; j < 4; j++) {
            if (i*4 + j < static_cast<int>(prevhash_bytes.size())) {
                job.header_template[4 + i*4 + (3-j)] = prevhash_bytes[i*4 + j];
            }
        }
    }

    // Merkle root (32 bytes) - computed from coinbase + merkle branch
    // For now, use coinbase hash directly (simplified)
    std::string coinbase = job.coinb1 + extranonce1_ + extranonce2 + job.coinb2;
    auto coinbase_bytes = hex_to_bytes(coinbase);

    // Double SHA-256 of coinbase
    uint32_t state[8];
    std::memcpy(state, sha256::H_INIT.data(), 32);

    // Hash coinbase in 64-byte chunks
    size_t offset = 0;
    while (offset + 64 <= coinbase_bytes.size()) {
        sha256::transform(state, coinbase_bytes.data() + offset);
        offset += 64;
    }

    // Final block with padding
    uint8_t final_block[64];
    std::memset(final_block, 0, 64);
    size_t remaining = coinbase_bytes.size() - offset;
    if (remaining > 0) {
        std::memcpy(final_block, coinbase_bytes.data() + offset, remaining);
    }
    final_block[remaining] = 0x80;

    if (remaining >= 56) {
        sha256::transform(state, final_block);
        std::memset(final_block, 0, 64);
    }

    uint64_t bit_len = coinbase_bytes.size() * 8;
    final_block[63] = bit_len & 0xff;
    final_block[62] = (bit_len >> 8) & 0xff;
    final_block[61] = (bit_len >> 16) & 0xff;
    final_block[60] = (bit_len >> 24) & 0xff;
    sha256::transform(state, final_block);

    // Second SHA-256
    uint8_t first_hash[32];
    for (int i = 0; i < 8; i++) {
        first_hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        first_hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        first_hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        first_hash[i*4 + 3] = state[i] & 0xff;
    }

    std::memcpy(state, sha256::H_INIT.data(), 32);
    std::memset(final_block, 0, 64);
    std::memcpy(final_block, first_hash, 32);
    final_block[32] = 0x80;
    final_block[62] = 0x01;
    sha256::transform(state, final_block);

    // Merkle root = coinbase hash (if no merkle branch) in little-endian
    for (int i = 0; i < 8; i++) {
        job.header_template[36 + i*4 + 0] = state[i] & 0xff;
        job.header_template[36 + i*4 + 1] = (state[i] >> 8) & 0xff;
        job.header_template[36 + i*4 + 2] = (state[i] >> 16) & 0xff;
        job.header_template[36 + i*4 + 3] = (state[i] >> 24) & 0xff;
    }

    // Time (4 bytes, little-endian)
    auto ntime_bytes = hex_to_bytes(job.ntime);
    for (int i = 0; i < 4 && i < static_cast<int>(ntime_bytes.size()); i++) {
        job.header_template[68 + (3-i)] = ntime_bytes[i];
    }

    // nBits (4 bytes, little-endian)
    auto nbits_bytes = hex_to_bytes(job.nbits);
    for (int i = 0; i < 4 && i < static_cast<int>(nbits_bytes.size()); i++) {
        job.header_template[72 + (3-i)] = nbits_bytes[i];
    }

    // Nonce placeholder at offset 76 (4 bytes) - will be filled by miner
}

void Client::calculate_target(Job& job) {
    // Decode compact target (nBits) to full 256-bit target
    auto nbits_bytes = hex_to_bytes(job.nbits);
    uint32_t nbits = (nbits_bytes[0] << 24) | (nbits_bytes[1] << 16) |
                     (nbits_bytes[2] << 8) | nbits_bytes[3];

    uint32_t exponent = nbits >> 24;
    uint32_t mantissa = nbits & 0x007fffff;

    std::memset(job.target, 0, 32);

    if (exponent <= 3) {
        mantissa >>= 8 * (3 - exponent);
        job.target[0] = mantissa & 0xff;
        job.target[1] = (mantissa >> 8) & 0xff;
        job.target[2] = (mantissa >> 16) & 0xff;
    } else {
        int offset = exponent - 3;
        if (offset < 32) {
            job.target[offset] = mantissa & 0xff;
            if (offset + 1 < 32) job.target[offset + 1] = (mantissa >> 8) & 0xff;
            if (offset + 2 < 32) job.target[offset + 2] = (mantissa >> 16) & 0xff;
        }
    }
}

bool Client::submit(const std::string& job_id, const std::string& extranonce2,
                    const std::string& ntime, const std::string& nonce) {
    std::ostringstream oss;
    oss << "{\"id\": " << msg_id_++ << ", \"method\": \"mining.submit\", "
        << "\"params\": [\"" << user_ << "\", \"" << job_id << "\", \""
        << extranonce2 << "\", \"" << ntime << "\", \"" << nonce << "\"]}";

    std::cout << "[Stratum] Submitting share: " << oss.str() << std::endl;
    return send_json(oss.str());
}

} // namespace stratum
