#pragma once

#include <string>
#include <iostream>

namespace colors {

// Global color enable flag
inline bool enabled = true;

// ANSI color codes
namespace codes {
    constexpr const char* RESET     = "\033[0m";
    constexpr const char* BOLD      = "\033[1m";
    constexpr const char* DIM       = "\033[2m";

    // Foreground colors
    constexpr const char* BLACK     = "\033[30m";
    constexpr const char* RED       = "\033[31m";
    constexpr const char* GREEN     = "\033[32m";
    constexpr const char* YELLOW    = "\033[33m";
    constexpr const char* BLUE      = "\033[34m";
    constexpr const char* MAGENTA   = "\033[35m";
    constexpr const char* CYAN      = "\033[36m";
    constexpr const char* WHITE     = "\033[37m";

    // Bright foreground colors
    constexpr const char* BRIGHT_BLACK   = "\033[90m";
    constexpr const char* BRIGHT_RED     = "\033[91m";
    constexpr const char* BRIGHT_GREEN   = "\033[92m";
    constexpr const char* BRIGHT_YELLOW  = "\033[93m";
    constexpr const char* BRIGHT_BLUE    = "\033[94m";
    constexpr const char* BRIGHT_MAGENTA = "\033[95m";
    constexpr const char* BRIGHT_CYAN    = "\033[96m";
    constexpr const char* BRIGHT_WHITE   = "\033[97m";
}

// Color wrapper functions that respect the enabled flag
inline std::string reset()   { return enabled ? codes::RESET : ""; }
inline std::string bold()    { return enabled ? codes::BOLD : ""; }
inline std::string dim()     { return enabled ? codes::DIM : ""; }

inline std::string red()     { return enabled ? codes::RED : ""; }
inline std::string green()   { return enabled ? codes::GREEN : ""; }
inline std::string yellow()  { return enabled ? codes::YELLOW : ""; }
inline std::string blue()    { return enabled ? codes::BLUE : ""; }
inline std::string magenta() { return enabled ? codes::MAGENTA : ""; }
inline std::string cyan()    { return enabled ? codes::CYAN : ""; }
inline std::string white()   { return enabled ? codes::WHITE : ""; }

inline std::string bright_red()     { return enabled ? codes::BRIGHT_RED : ""; }
inline std::string bright_green()   { return enabled ? codes::BRIGHT_GREEN : ""; }
inline std::string bright_yellow()  { return enabled ? codes::BRIGHT_YELLOW : ""; }
inline std::string bright_blue()    { return enabled ? codes::BRIGHT_BLUE : ""; }
inline std::string bright_cyan()    { return enabled ? codes::BRIGHT_CYAN : ""; }

// Semantic colors for log messages
inline std::string error()   { return enabled ? codes::BRIGHT_RED : ""; }
inline std::string warn()    { return enabled ? codes::YELLOW : ""; }
inline std::string success() { return enabled ? codes::BRIGHT_GREEN : ""; }
inline std::string info()    { return enabled ? codes::BRIGHT_CYAN : ""; }
inline std::string tag()     { return enabled ? codes::CYAN : ""; }
inline std::string value()   { return enabled ? codes::BRIGHT_WHITE : ""; }
inline std::string muted()   { return enabled ? codes::BRIGHT_BLACK : ""; }

// Helper to wrap text in color
inline std::string colorize(const std::string& text, const char* color_code) {
    if (!enabled) return text;
    return std::string(color_code) + text + codes::RESET;
}

// Log prefix helpers
inline std::string log_prefix(const std::string& component) {
    if (!enabled) return "[" + component + "]";
    return codes::CYAN + std::string("[") + codes::BRIGHT_CYAN + component +
           codes::CYAN + "]" + codes::RESET;
}

} // namespace colors
