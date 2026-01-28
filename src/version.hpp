#pragma once

// BTC Miner Version Information
// This file is auto-updated by CI/CD during release builds

#define BTCMINER_VERSION_MAJOR 0
#define BTCMINER_VERSION_MINOR 1
#define BTCMINER_VERSION_PATCH 0

#define BTCMINER_VERSION_STRING "0.1.0"

// Git commit hash (set by CI/CD or build system)
#ifndef BTCMINER_GIT_HASH
#define BTCMINER_GIT_HASH "unknown"
#endif

// Build type
#ifndef BTCMINER_BUILD_TYPE
#define BTCMINER_BUILD_TYPE "dev"
#endif

namespace version {

inline const char* get_version() {
    return BTCMINER_VERSION_STRING;
}

inline const char* get_git_hash() {
    return BTCMINER_GIT_HASH;
}

inline const char* get_build_type() {
    return BTCMINER_BUILD_TYPE;
}

inline const char* get_full_version() {
    static char buf[128];
    snprintf(buf, sizeof(buf), "%s-%s (%s)",
             BTCMINER_VERSION_STRING, BTCMINER_BUILD_TYPE, BTCMINER_GIT_HASH);
    return buf;
}

} // namespace version
