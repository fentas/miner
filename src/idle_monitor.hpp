#pragma once

#include <atomic>
#include <thread>
#include <functional>
#include <chrono>
#include <fstream>
#include <sstream>
#include <iostream>
#include <cstdlib>
#include <array>
#include <memory>

namespace idle {

enum class Environment {
    Unknown,
    NativeLinux,    // X11 or Wayland
    WSL2,           // Windows Subsystem for Linux 2
    Headless        // No display (SSH, server)
};

class IdleMonitor {
public:
    using IdleCallback = std::function<void(bool is_idle)>;

    IdleMonitor() : running_(false), currently_idle_(false), idle_timeout_sec_(900) {
        detect_environment();
    }

    ~IdleMonitor() {
        stop();
    }

    void set_timeout(unsigned seconds) {
        idle_timeout_sec_ = seconds;
    }

    void set_callback(IdleCallback cb) {
        callback_ = cb;
    }

    void start() {
        if (running_ || idle_timeout_sec_ == 0) return;

        if (env_ == Environment::Headless || env_ == Environment::Unknown) {
            std::cout << "[Idle] Idle monitoring not available (headless/unknown environment)" << std::endl;
            return;
        }

        running_ = true;
        monitor_thread_ = std::thread(&IdleMonitor::monitor_loop, this);
        std::cout << "[Idle] Monitor started (timeout: " << idle_timeout_sec_ << "s, env: "
                  << env_to_string(env_) << ")" << std::endl;
    }

    void stop() {
        running_ = false;
        if (monitor_thread_.joinable()) {
            monitor_thread_.join();
        }
    }

    bool is_idle() const { return currently_idle_; }
    Environment get_environment() const { return env_; }
    unsigned get_idle_time() const { return last_idle_time_; }

    static std::string env_to_string(Environment env) {
        switch (env) {
            case Environment::NativeLinux: return "linux";
            case Environment::WSL2: return "wsl2";
            case Environment::Headless: return "headless";
            default: return "unknown";
        }
    }

private:
    void detect_environment() {
        // Check for WSL2
        std::ifstream proc_version("/proc/version");
        if (proc_version) {
            std::string line;
            std::getline(proc_version, line);
            if (line.find("microsoft") != std::string::npos ||
                line.find("Microsoft") != std::string::npos ||
                line.find("WSL") != std::string::npos) {
                env_ = Environment::WSL2;
                std::cout << "[Idle] Detected WSL2 environment" << std::endl;
                return;
            }
        }

        // Check for display (X11/Wayland)
        const char* display = std::getenv("DISPLAY");
        const char* wayland = std::getenv("WAYLAND_DISPLAY");

        if (display || wayland) {
            env_ = Environment::NativeLinux;
            std::cout << "[Idle] Detected native Linux with display" << std::endl;
            return;
        }

        env_ = Environment::Headless;
        std::cout << "[Idle] Detected headless environment" << std::endl;
    }

    unsigned get_idle_time_ms() {
        switch (env_) {
            case Environment::WSL2:
                return get_windows_idle_time();
            case Environment::NativeLinux:
                return get_x11_idle_time();
            default:
                return 0;
        }
    }

    unsigned get_windows_idle_time() {
        // Query Windows idle time via PowerShell
        // Uses GetLastInputInfo Windows API
        static const char* ps_cmd =
            "powershell.exe -NoProfile -NonInteractive -Command \""
            "$code = @'\n"
            "using System;\n"
            "using System.Runtime.InteropServices;\n"
            "public class Win32 {\n"
            "    [DllImport(\\\"user32.dll\\\")]\n"
            "    public static extern bool GetLastInputInfo(ref LASTINPUTINFO plii);\n"
            "    public struct LASTINPUTINFO { public uint cbSize; public uint dwTime; }\n"
            "    public static uint GetIdleTime() {\n"
            "        LASTINPUTINFO lii = new LASTINPUTINFO();\n"
            "        lii.cbSize = (uint)Marshal.SizeOf(lii);\n"
            "        GetLastInputInfo(ref lii);\n"
            "        return (uint)Environment.TickCount - lii.dwTime;\n"
            "    }\n"
            "}\n"
            "'@\n"
            "Add-Type -TypeDefinition $code -Language CSharp 2>$null\n"
            "[Win32]::GetIdleTime()\n"
            "\" 2>/dev/null";

        std::string result = exec_command(ps_cmd);
        if (!result.empty()) {
            try {
                return static_cast<unsigned>(std::stoul(result));
            } catch (...) {}
        }
        return 0;
    }

    unsigned get_x11_idle_time() {
        // Try xprintidle first (most common)
        std::string result = exec_command("xprintidle 2>/dev/null");
        if (!result.empty()) {
            try {
                return static_cast<unsigned>(std::stoul(result));
            } catch (...) {}
        }

        // Fallback: try xssstate (from slock/suckless)
        result = exec_command("xssstate -i 2>/dev/null");
        if (!result.empty()) {
            try {
                return static_cast<unsigned>(std::stoul(result));
            } catch (...) {}
        }

        // If no tool available, return 0 (always active)
        return 0;
    }

    std::string exec_command(const char* cmd) {
        std::array<char, 128> buffer;
        std::string result;
        std::unique_ptr<FILE, decltype(&pclose)> pipe(popen(cmd, "r"), pclose);
        if (!pipe) return "";

        while (fgets(buffer.data(), buffer.size(), pipe.get()) != nullptr) {
            result += buffer.data();
        }

        // Trim whitespace
        while (!result.empty() && (result.back() == '\n' || result.back() == '\r' || result.back() == ' ')) {
            result.pop_back();
        }
        return result;
    }

    void monitor_loop() {
        while (running_) {
            unsigned idle_ms = get_idle_time_ms();
            last_idle_time_ = idle_ms / 1000;  // Store in seconds

            bool should_be_idle = (idle_ms / 1000) >= idle_timeout_sec_;

            if (should_be_idle != currently_idle_) {
                currently_idle_ = should_be_idle;
                if (callback_) {
                    callback_(currently_idle_);
                }
                std::cout << "[Idle] User " << (currently_idle_ ? "idle" : "active")
                          << " (idle time: " << (idle_ms / 1000) << "s)" << std::endl;
            }

            // Check every 5 seconds
            std::this_thread::sleep_for(std::chrono::seconds(5));
        }
    }

    Environment env_ = Environment::Unknown;
    std::atomic<bool> running_;
    std::atomic<bool> currently_idle_;
    std::atomic<unsigned> last_idle_time_{0};
    unsigned idle_timeout_sec_;
    std::thread monitor_thread_;
    IdleCallback callback_;
};

} // namespace idle
