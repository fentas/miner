#pragma once

#include <string>
#include <map>
#include <vector>
#include <functional>
#include <thread>
#include <mutex>
#include <atomic>
#include <set>
#include <sstream>
#include <fstream>
#include <cstring>
#include <algorithm>

#include <sys/socket.h>
#include <netinet/in.h>
#include <unistd.h>
#include <arpa/inet.h>

namespace web {

// Base64 encoding for WebSocket handshake
inline std::string base64_encode(const unsigned char* data, size_t len) {
    static const char* chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    std::string result;
    int val = 0, valb = -6;
    for (size_t i = 0; i < len; i++) {
        val = (val << 8) + data[i];
        valb += 8;
        while (valb >= 0) {
            result.push_back(chars[(val >> valb) & 0x3F]);
            valb -= 6;
        }
    }
    if (valb > -6) result.push_back(chars[((val << 8) >> (valb + 8)) & 0x3F]);
    while (result.size() % 4) result.push_back('=');
    return result;
}

// Simple SHA-1 for WebSocket (minimal implementation)
inline void sha1(const uint8_t* data, size_t len, uint8_t* hash) {
    uint32_t h0 = 0x67452301, h1 = 0xEFCDAB89, h2 = 0x98BADCFE, h3 = 0x10325476, h4 = 0xC3D2E1F0;

    // Pad message
    size_t ml = len * 8;
    size_t padded_len = ((len + 8) / 64 + 1) * 64;
    std::vector<uint8_t> msg(padded_len, 0);
    memcpy(msg.data(), data, len);
    msg[len] = 0x80;
    for (int i = 0; i < 8; i++) msg[padded_len - 1 - i] = (ml >> (i * 8)) & 0xff;

    // Process blocks
    for (size_t offset = 0; offset < padded_len; offset += 64) {
        uint32_t w[80];
        for (int i = 0; i < 16; i++) {
            w[i] = (msg[offset + i*4] << 24) | (msg[offset + i*4 + 1] << 16) |
                   (msg[offset + i*4 + 2] << 8) | msg[offset + i*4 + 3];
        }
        for (int i = 16; i < 80; i++) {
            uint32_t t = w[i-3] ^ w[i-8] ^ w[i-14] ^ w[i-16];
            w[i] = (t << 1) | (t >> 31);
        }

        uint32_t a = h0, b = h1, c = h2, d = h3, e = h4;
        for (int i = 0; i < 80; i++) {
            uint32_t f, k;
            if (i < 20) { f = (b & c) | ((~b) & d); k = 0x5A827999; }
            else if (i < 40) { f = b ^ c ^ d; k = 0x6ED9EBA1; }
            else if (i < 60) { f = (b & c) | (b & d) | (c & d); k = 0x8F1BBCDC; }
            else { f = b ^ c ^ d; k = 0xCA62C1D6; }
            uint32_t t = ((a << 5) | (a >> 27)) + f + e + k + w[i];
            e = d; d = c; c = (b << 30) | (b >> 2); b = a; a = t;
        }
        h0 += a; h1 += b; h2 += c; h3 += d; h4 += e;
    }

    hash[0] = h0 >> 24; hash[1] = h0 >> 16; hash[2] = h0 >> 8; hash[3] = h0;
    hash[4] = h1 >> 24; hash[5] = h1 >> 16; hash[6] = h1 >> 8; hash[7] = h1;
    hash[8] = h2 >> 24; hash[9] = h2 >> 16; hash[10] = h2 >> 8; hash[11] = h2;
    hash[12] = h3 >> 24; hash[13] = h3 >> 16; hash[14] = h3 >> 8; hash[15] = h3;
    hash[16] = h4 >> 24; hash[17] = h4 >> 16; hash[18] = h4 >> 8; hash[19] = h4;
}

struct Request {
    std::string method;
    std::string path;
    std::map<std::string, std::string> headers;
    std::string body;
};

struct Response {
    int status = 200;
    std::string status_text = "OK";
    std::map<std::string, std::string> headers;
    std::string body;

    void json(const std::string& data) {
        headers["Content-Type"] = "application/json";
        body = data;
    }

    void html(const std::string& data) {
        headers["Content-Type"] = "text/html; charset=utf-8";
        body = data;
    }

    void file(const std::string& content, const std::string& mime) {
        headers["Content-Type"] = mime;
        body = content;
    }
};

using RouteHandler = std::function<void(const Request&, Response&)>;

class WebServer {
public:
    WebServer(uint16_t port = 8080) : port_(port) {}
    ~WebServer() { stop(); }

    void get(const std::string& path, RouteHandler handler) {
        routes_["GET:" + path] = handler;
    }

    void post(const std::string& path, RouteHandler handler) {
        routes_["POST:" + path] = handler;
    }

    void set_static_dir(const std::string& dir) {
        static_dir_ = dir;
    }

    // Broadcast to all WebSocket clients
    void broadcast(const std::string& message) {
        std::lock_guard<std::mutex> lock(ws_mutex_);
        for (int fd : ws_clients_) {
            send_ws_frame(fd, message);
        }
    }

    bool start() {
        server_fd_ = socket(AF_INET, SOCK_STREAM, 0);
        if (server_fd_ < 0) return false;

        int opt = 1;
        setsockopt(server_fd_, SOL_SOCKET, SO_REUSEADDR, &opt, sizeof(opt));

        struct sockaddr_in addr;
        addr.sin_family = AF_INET;
        addr.sin_addr.s_addr = INADDR_ANY;
        addr.sin_port = htons(port_);

        if (bind(server_fd_, (struct sockaddr*)&addr, sizeof(addr)) < 0) {
            close(server_fd_);
            return false;
        }

        if (listen(server_fd_, 10) < 0) {
            close(server_fd_);
            return false;
        }

        running_ = true;
        server_thread_ = std::thread(&WebServer::accept_loop, this);
        return true;
    }

    void stop() {
        running_ = false;
        if (server_fd_ >= 0) {
            close(server_fd_);
            server_fd_ = -1;
        }
        if (server_thread_.joinable()) {
            server_thread_.join();
        }
        // Close WebSocket clients
        std::lock_guard<std::mutex> lock(ws_mutex_);
        for (int fd : ws_clients_) {
            close(fd);
        }
        ws_clients_.clear();
    }

    uint16_t port() const { return port_; }

private:
    void accept_loop() {
        while (running_) {
            struct sockaddr_in client_addr;
            socklen_t client_len = sizeof(client_addr);

            int client_fd = accept(server_fd_, (struct sockaddr*)&client_addr, &client_len);
            if (client_fd < 0) continue;

            // Handle in new thread
            std::thread(&WebServer::handle_client, this, client_fd).detach();
        }
    }

    void handle_client(int fd) {
        char buffer[8192];
        ssize_t n = recv(fd, buffer, sizeof(buffer) - 1, 0);
        if (n <= 0) {
            close(fd);
            return;
        }
        buffer[n] = '\0';

        Request req = parse_request(buffer);

        // Check for WebSocket upgrade
        if (req.headers.count("Upgrade") && req.headers["Upgrade"] == "websocket") {
            handle_websocket(fd, req);
            return;
        }

        Response res;

        // Try routes
        std::string route_key = req.method + ":" + req.path;
        if (routes_.count(route_key)) {
            routes_[route_key](req, res);
        } else if (!static_dir_.empty()) {
            serve_static(req, res);
        } else {
            res.status = 404;
            res.status_text = "Not Found";
            res.body = "404 Not Found";
        }

        send_response(fd, res);
        close(fd);
    }

    Request parse_request(const char* data) {
        Request req;
        std::istringstream stream(data);
        std::string line;

        // Request line
        std::getline(stream, line);
        std::istringstream req_line(line);
        req_line >> req.method >> req.path;

        // Remove query string for routing
        size_t q = req.path.find('?');
        if (q != std::string::npos) req.path = req.path.substr(0, q);

        // Headers
        while (std::getline(stream, line) && line != "\r" && !line.empty()) {
            if (line.back() == '\r') line.pop_back();
            size_t colon = line.find(':');
            if (colon != std::string::npos) {
                std::string key = line.substr(0, colon);
                std::string val = line.substr(colon + 1);
                while (!val.empty() && val[0] == ' ') val.erase(0, 1);
                req.headers[key] = val;
            }
        }

        // Body
        std::ostringstream body;
        body << stream.rdbuf();
        req.body = body.str();

        return req;
    }

    void send_response(int fd, const Response& res) {
        std::ostringstream ss;
        ss << "HTTP/1.1 " << res.status << " " << res.status_text << "\r\n";
        ss << "Content-Length: " << res.body.size() << "\r\n";
        ss << "Connection: close\r\n";
        ss << "Access-Control-Allow-Origin: *\r\n";
        for (const auto& h : res.headers) {
            ss << h.first << ": " << h.second << "\r\n";
        }
        ss << "\r\n";
        ss << res.body;

        std::string response = ss.str();
        send(fd, response.c_str(), response.size(), 0);
    }

    void serve_static(const Request& req, Response& res) {
        std::string path = req.path;
        if (path == "/") path = "/index.html";

        std::string filepath = static_dir_ + path;
        std::ifstream file(filepath, std::ios::binary);

        if (!file) {
            // Try index.html for SPA routing
            filepath = static_dir_ + "/index.html";
            file.open(filepath, std::ios::binary);
        }

        if (!file) {
            res.status = 404;
            res.status_text = "Not Found";
            res.body = "404 Not Found";
            return;
        }

        std::ostringstream ss;
        ss << file.rdbuf();

        // Determine MIME type
        std::string mime = "text/plain";
        if (path.find(".html") != std::string::npos) mime = "text/html; charset=utf-8";
        else if (path.find(".css") != std::string::npos) mime = "text/css";
        else if (path.find(".js") != std::string::npos) mime = "application/javascript";
        else if (path.find(".json") != std::string::npos) mime = "application/json";
        else if (path.find(".png") != std::string::npos) mime = "image/png";
        else if (path.find(".svg") != std::string::npos) mime = "image/svg+xml";
        else if (path.find(".ico") != std::string::npos) mime = "image/x-icon";

        res.file(ss.str(), mime);
    }

    void handle_websocket(int fd, const Request& req) {
        // WebSocket handshake
        std::string key = req.headers.count("Sec-WebSocket-Key") ?
                          req.headers.at("Sec-WebSocket-Key") : "";

        std::string magic = key + "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
        uint8_t hash[20];
        sha1(reinterpret_cast<const uint8_t*>(magic.c_str()), magic.size(), hash);
        std::string accept = base64_encode(hash, 20);

        std::ostringstream ss;
        ss << "HTTP/1.1 101 Switching Protocols\r\n";
        ss << "Upgrade: websocket\r\n";
        ss << "Connection: Upgrade\r\n";
        ss << "Sec-WebSocket-Accept: " << accept << "\r\n";
        ss << "\r\n";

        std::string response = ss.str();
        send(fd, response.c_str(), response.size(), 0);

        // Add to clients
        {
            std::lock_guard<std::mutex> lock(ws_mutex_);
            ws_clients_.insert(fd);
        }

        // Keep connection open, read frames
        while (running_) {
            uint8_t header[2];
            if (recv(fd, header, 2, 0) <= 0) break;

            bool fin = header[0] & 0x80;
            uint8_t opcode = header[0] & 0x0F;
            bool masked = header[1] & 0x80;
            uint64_t len = header[1] & 0x7F;

            if (len == 126) {
                uint8_t ext[2];
                recv(fd, ext, 2, 0);
                len = (ext[0] << 8) | ext[1];
            } else if (len == 127) {
                uint8_t ext[8];
                recv(fd, ext, 8, 0);
                len = 0;
                for (int i = 0; i < 8; i++) len = (len << 8) | ext[i];
            }

            uint8_t mask[4] = {0};
            if (masked) recv(fd, mask, 4, 0);

            std::vector<uint8_t> payload(len);
            if (len > 0) recv(fd, payload.data(), len, 0);

            if (masked) {
                for (size_t i = 0; i < len; i++) {
                    payload[i] ^= mask[i % 4];
                }
            }

            // Handle close frame
            if (opcode == 0x08) break;

            // Handle ping - respond with pong
            if (opcode == 0x09) {
                send_ws_frame(fd, std::string(payload.begin(), payload.end()), 0x0A);
            }

            (void)fin; // Suppress warning
        }

        // Remove from clients
        {
            std::lock_guard<std::mutex> lock(ws_mutex_);
            ws_clients_.erase(fd);
        }
        close(fd);
    }

    void send_ws_frame(int fd, const std::string& data, uint8_t opcode = 0x01) {
        std::vector<uint8_t> frame;
        frame.push_back(0x80 | opcode);  // FIN + opcode

        if (data.size() < 126) {
            frame.push_back(static_cast<uint8_t>(data.size()));
        } else if (data.size() < 65536) {
            frame.push_back(126);
            frame.push_back((data.size() >> 8) & 0xFF);
            frame.push_back(data.size() & 0xFF);
        } else {
            frame.push_back(127);
            for (int i = 7; i >= 0; i--) {
                frame.push_back((data.size() >> (i * 8)) & 0xFF);
            }
        }

        frame.insert(frame.end(), data.begin(), data.end());
        send(fd, frame.data(), frame.size(), MSG_NOSIGNAL);
    }

    uint16_t port_;
    int server_fd_ = -1;
    std::atomic<bool> running_{false};
    std::thread server_thread_;

    std::map<std::string, RouteHandler> routes_;
    std::string static_dir_;

    std::mutex ws_mutex_;
    std::set<int> ws_clients_;
};

} // namespace web
