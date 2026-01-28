#pragma once

// AVX2/AVX512 Multi-hashing for Bitcoin Mining
// Processes 8 (AVX2) or 16 (AVX512) nonces simultaneously

#include <cstdint>
#include <cstring>
#include <array>

#if defined(HAVE_AVX2) || defined(HAVE_AVX512)
#include <immintrin.h>
#endif

#include "sha256.hpp"

namespace sha256 {

#ifdef HAVE_AVX2

// AVX2: Process 8 nonces in parallel using 256-bit registers
// Each lane computes SHA-256 independently

// Rotate right for 8x32-bit values
inline __m256i rotr_avx2(__m256i x, int n) {
    return _mm256_or_si256(_mm256_srli_epi32(x, n), _mm256_slli_epi32(x, 32 - n));
}

// SHA-256 functions for 8 parallel lanes
inline __m256i ch_avx2(__m256i x, __m256i y, __m256i z) {
    return _mm256_xor_si256(_mm256_and_si256(x, y), _mm256_andnot_si256(x, z));
}

inline __m256i maj_avx2(__m256i x, __m256i y, __m256i z) {
    return _mm256_xor_si256(_mm256_xor_si256(_mm256_and_si256(x, y), _mm256_and_si256(x, z)), _mm256_and_si256(y, z));
}

inline __m256i sigma0_avx2(__m256i x) {
    return _mm256_xor_si256(_mm256_xor_si256(rotr_avx2(x, 2), rotr_avx2(x, 13)), rotr_avx2(x, 22));
}

inline __m256i sigma1_avx2(__m256i x) {
    return _mm256_xor_si256(_mm256_xor_si256(rotr_avx2(x, 6), rotr_avx2(x, 11)), rotr_avx2(x, 25));
}

inline __m256i gamma0_avx2(__m256i x) {
    return _mm256_xor_si256(_mm256_xor_si256(rotr_avx2(x, 7), rotr_avx2(x, 18)), _mm256_srli_epi32(x, 3));
}

inline __m256i gamma1_avx2(__m256i x) {
    return _mm256_xor_si256(_mm256_xor_si256(rotr_avx2(x, 17), rotr_avx2(x, 19)), _mm256_srli_epi32(x, 10));
}

// SHA-256 round constants broadcast to all 8 lanes
inline __m256i K_avx2(int i) {
    return _mm256_set1_epi32(K[i]);
}

// AVX2 SHA-256 transform for 8 parallel blocks
// state: 8x8 uint32_t values (8 lanes, 8 state words each)
// blocks: 8 blocks of 64 bytes each
inline void transform_avx2_8way(uint32_t state[8][8], const uint8_t blocks[8][64]) {
    __m256i W[64];
    __m256i a, b, c, d, e, f, g, h;

    // Load state - transpose from state[word][lane] to 8 ymm registers
    a = _mm256_set_epi32(state[0][7], state[0][6], state[0][5], state[0][4],
                         state[0][3], state[0][2], state[0][1], state[0][0]);
    b = _mm256_set_epi32(state[1][7], state[1][6], state[1][5], state[1][4],
                         state[1][3], state[1][2], state[1][1], state[1][0]);
    c = _mm256_set_epi32(state[2][7], state[2][6], state[2][5], state[2][4],
                         state[2][3], state[2][2], state[2][1], state[2][0]);
    d = _mm256_set_epi32(state[3][7], state[3][6], state[3][5], state[3][4],
                         state[3][3], state[3][2], state[3][1], state[3][0]);
    e = _mm256_set_epi32(state[4][7], state[4][6], state[4][5], state[4][4],
                         state[4][3], state[4][2], state[4][1], state[4][0]);
    f = _mm256_set_epi32(state[5][7], state[5][6], state[5][5], state[5][4],
                         state[5][3], state[5][2], state[5][1], state[5][0]);
    g = _mm256_set_epi32(state[6][7], state[6][6], state[6][5], state[6][4],
                         state[6][3], state[6][2], state[6][1], state[6][0]);
    h = _mm256_set_epi32(state[7][7], state[7][6], state[7][5], state[7][4],
                         state[7][3], state[7][2], state[7][1], state[7][0]);

    __m256i a_save = a, b_save = b, c_save = c, d_save = d;
    __m256i e_save = e, f_save = f, g_save = g, h_save = h;

    // Prepare message schedule - load from 8 blocks and byte-swap
    for (int i = 0; i < 16; i++) {
        // Load word i from each of the 8 blocks
        uint32_t words[8];
        for (int lane = 0; lane < 8; lane++) {
            uint32_t w;
            std::memcpy(&w, &blocks[lane][i * 4], 4);
            words[lane] = bswap32(w);
        }
        W[i] = _mm256_set_epi32(words[7], words[6], words[5], words[4],
                                 words[3], words[2], words[1], words[0]);
    }

    // Message schedule extension
    for (int i = 16; i < 64; i++) {
        W[i] = _mm256_add_epi32(_mm256_add_epi32(gamma1_avx2(W[i-2]), W[i-7]),
                                _mm256_add_epi32(gamma0_avx2(W[i-15]), W[i-16]));
    }

    // 64 rounds
    for (int i = 0; i < 64; i++) {
        __m256i T1 = _mm256_add_epi32(
            _mm256_add_epi32(h, sigma1_avx2(e)),
            _mm256_add_epi32(_mm256_add_epi32(ch_avx2(e, f, g), K_avx2(i)), W[i]));
        __m256i T2 = _mm256_add_epi32(sigma0_avx2(a), maj_avx2(a, b, c));

        h = g; g = f; f = e;
        e = _mm256_add_epi32(d, T1);
        d = c; c = b; b = a;
        a = _mm256_add_epi32(T1, T2);
    }

    // Add to saved state
    a = _mm256_add_epi32(a, a_save);
    b = _mm256_add_epi32(b, b_save);
    c = _mm256_add_epi32(c, c_save);
    d = _mm256_add_epi32(d, d_save);
    e = _mm256_add_epi32(e, e_save);
    f = _mm256_add_epi32(f, f_save);
    g = _mm256_add_epi32(g, g_save);
    h = _mm256_add_epi32(h, h_save);

    // Store back to state array
    alignas(32) uint32_t temp[8];

    _mm256_store_si256((__m256i*)temp, a);
    for (int lane = 0; lane < 8; lane++) state[0][lane] = temp[lane];

    _mm256_store_si256((__m256i*)temp, b);
    for (int lane = 0; lane < 8; lane++) state[1][lane] = temp[lane];

    _mm256_store_si256((__m256i*)temp, c);
    for (int lane = 0; lane < 8; lane++) state[2][lane] = temp[lane];

    _mm256_store_si256((__m256i*)temp, d);
    for (int lane = 0; lane < 8; lane++) state[3][lane] = temp[lane];

    _mm256_store_si256((__m256i*)temp, e);
    for (int lane = 0; lane < 8; lane++) state[4][lane] = temp[lane];

    _mm256_store_si256((__m256i*)temp, f);
    for (int lane = 0; lane < 8; lane++) state[5][lane] = temp[lane];

    _mm256_store_si256((__m256i*)temp, g);
    for (int lane = 0; lane < 8; lane++) state[6][lane] = temp[lane];

    _mm256_store_si256((__m256i*)temp, h);
    for (int lane = 0; lane < 8; lane++) state[7][lane] = temp[lane];
}

// Process 8 nonces with midstate optimization
// Returns bitmask of which nonces found valid hashes (meet target)
inline uint8_t sha256d_midstate_8way(const Midstate& ms, uint32_t nonce_base,
                                      const uint8_t target[32], uint8_t hashes[8][32]) {
    alignas(32) uint32_t state[8][8];  // [state_word][lane]
    alignas(32) uint8_t blocks[8][64];
    alignas(32) uint8_t first_hash[8][32];

    // Initialize state from midstate for all 8 lanes
    for (int lane = 0; lane < 8; lane++) {
        for (int i = 0; i < 8; i++) {
            state[i][lane] = ms.state[i];
        }
    }

    // Build 8 blocks with consecutive nonces
    for (int lane = 0; lane < 8; lane++) {
        std::memset(blocks[lane], 0, 64);
        std::memcpy(blocks[lane], ms.tail, 12);

        uint32_t nonce = nonce_base + lane;
        blocks[lane][12] = nonce & 0xff;
        blocks[lane][13] = (nonce >> 8) & 0xff;
        blocks[lane][14] = (nonce >> 16) & 0xff;
        blocks[lane][15] = (nonce >> 24) & 0xff;

        blocks[lane][16] = 0x80;  // Padding
        blocks[lane][62] = 0x02;  // Length high byte (640 bits)
        blocks[lane][63] = 0x80;  // Length low byte
    }

    // First transform (complete first SHA-256)
    transform_avx2_8way(state, blocks);

    // Convert first hash to bytes (big-endian) for each lane
    for (int lane = 0; lane < 8; lane++) {
        for (int i = 0; i < 8; i++) {
            first_hash[lane][i*4 + 0] = (state[i][lane] >> 24) & 0xff;
            first_hash[lane][i*4 + 1] = (state[i][lane] >> 16) & 0xff;
            first_hash[lane][i*4 + 2] = (state[i][lane] >> 8) & 0xff;
            first_hash[lane][i*4 + 3] = state[i][lane] & 0xff;
        }
    }

    // Reset state to H_INIT for second hash
    for (int lane = 0; lane < 8; lane++) {
        for (int i = 0; i < 8; i++) {
            state[i][lane] = H_INIT[i];
        }
    }

    // Build blocks for second hash
    for (int lane = 0; lane < 8; lane++) {
        std::memset(blocks[lane], 0, 64);
        std::memcpy(blocks[lane], first_hash[lane], 32);
        blocks[lane][32] = 0x80;
        blocks[lane][62] = 0x01;  // Length = 256 bits
        blocks[lane][63] = 0x00;
    }

    // Second transform
    transform_avx2_8way(state, blocks);

    // Convert to little-endian output and check target
    uint8_t valid_mask = 0;

    for (int lane = 0; lane < 8; lane++) {
        for (int i = 0; i < 8; i++) {
            hashes[lane][i*4 + 3] = (state[i][lane] >> 24) & 0xff;
            hashes[lane][i*4 + 2] = (state[i][lane] >> 16) & 0xff;
            hashes[lane][i*4 + 1] = (state[i][lane] >> 8) & 0xff;
            hashes[lane][i*4 + 0] = state[i][lane] & 0xff;
        }

        // Check if hash meets target
        bool valid = true;
        for (int i = 31; i >= 0 && valid; i--) {
            if (hashes[lane][i] < target[i]) break;
            if (hashes[lane][i] > target[i]) valid = false;
        }
        if (valid) valid_mask |= (1 << lane);
    }

    return valid_mask;
}

#endif // HAVE_AVX2

#ifdef HAVE_AVX512

// AVX512: Process 16 nonces in parallel using 512-bit registers

inline __m512i rotr_avx512(__m512i x, int n) {
    return _mm512_or_si512(_mm512_srli_epi32(x, n), _mm512_slli_epi32(x, 32 - n));
}

inline __m512i ch_avx512(__m512i x, __m512i y, __m512i z) {
    return _mm512_xor_si512(_mm512_and_si512(x, y), _mm512_andnot_si512(x, z));
}

inline __m512i maj_avx512(__m512i x, __m512i y, __m512i z) {
    return _mm512_xor_si512(_mm512_xor_si512(_mm512_and_si512(x, y), _mm512_and_si512(x, z)), _mm512_and_si512(y, z));
}

inline __m512i sigma0_avx512(__m512i x) {
    return _mm512_xor_si512(_mm512_xor_si512(rotr_avx512(x, 2), rotr_avx512(x, 13)), rotr_avx512(x, 22));
}

inline __m512i sigma1_avx512(__m512i x) {
    return _mm512_xor_si512(_mm512_xor_si512(rotr_avx512(x, 6), rotr_avx512(x, 11)), rotr_avx512(x, 25));
}

inline __m512i gamma0_avx512(__m512i x) {
    return _mm512_xor_si512(_mm512_xor_si512(rotr_avx512(x, 7), rotr_avx512(x, 18)), _mm512_srli_epi32(x, 3));
}

inline __m512i gamma1_avx512(__m512i x) {
    return _mm512_xor_si512(_mm512_xor_si512(rotr_avx512(x, 17), rotr_avx512(x, 19)), _mm512_srli_epi32(x, 10));
}

inline __m512i K_avx512(int i) {
    return _mm512_set1_epi32(K[i]);
}

// AVX512 SHA-256 transform for 16 parallel blocks
inline void transform_avx512_16way(uint32_t state[8][16], const uint8_t blocks[16][64]) {
    __m512i W[64];
    __m512i a, b, c, d, e, f, g, h;

    // Load state
    a = _mm512_set_epi32(state[0][15], state[0][14], state[0][13], state[0][12],
                         state[0][11], state[0][10], state[0][9], state[0][8],
                         state[0][7], state[0][6], state[0][5], state[0][4],
                         state[0][3], state[0][2], state[0][1], state[0][0]);
    b = _mm512_set_epi32(state[1][15], state[1][14], state[1][13], state[1][12],
                         state[1][11], state[1][10], state[1][9], state[1][8],
                         state[1][7], state[1][6], state[1][5], state[1][4],
                         state[1][3], state[1][2], state[1][1], state[1][0]);
    c = _mm512_set_epi32(state[2][15], state[2][14], state[2][13], state[2][12],
                         state[2][11], state[2][10], state[2][9], state[2][8],
                         state[2][7], state[2][6], state[2][5], state[2][4],
                         state[2][3], state[2][2], state[2][1], state[2][0]);
    d = _mm512_set_epi32(state[3][15], state[3][14], state[3][13], state[3][12],
                         state[3][11], state[3][10], state[3][9], state[3][8],
                         state[3][7], state[3][6], state[3][5], state[3][4],
                         state[3][3], state[3][2], state[3][1], state[3][0]);
    e = _mm512_set_epi32(state[4][15], state[4][14], state[4][13], state[4][12],
                         state[4][11], state[4][10], state[4][9], state[4][8],
                         state[4][7], state[4][6], state[4][5], state[4][4],
                         state[4][3], state[4][2], state[4][1], state[4][0]);
    f = _mm512_set_epi32(state[5][15], state[5][14], state[5][13], state[5][12],
                         state[5][11], state[5][10], state[5][9], state[5][8],
                         state[5][7], state[5][6], state[5][5], state[5][4],
                         state[5][3], state[5][2], state[5][1], state[5][0]);
    g = _mm512_set_epi32(state[6][15], state[6][14], state[6][13], state[6][12],
                         state[6][11], state[6][10], state[6][9], state[6][8],
                         state[6][7], state[6][6], state[6][5], state[6][4],
                         state[6][3], state[6][2], state[6][1], state[6][0]);
    h = _mm512_set_epi32(state[7][15], state[7][14], state[7][13], state[7][12],
                         state[7][11], state[7][10], state[7][9], state[7][8],
                         state[7][7], state[7][6], state[7][5], state[7][4],
                         state[7][3], state[7][2], state[7][1], state[7][0]);

    __m512i a_save = a, b_save = b, c_save = c, d_save = d;
    __m512i e_save = e, f_save = f, g_save = g, h_save = h;

    // Prepare message schedule
    for (int i = 0; i < 16; i++) {
        uint32_t words[16];
        for (int lane = 0; lane < 16; lane++) {
            uint32_t w;
            std::memcpy(&w, &blocks[lane][i * 4], 4);
            words[lane] = bswap32(w);
        }
        W[i] = _mm512_set_epi32(words[15], words[14], words[13], words[12],
                                 words[11], words[10], words[9], words[8],
                                 words[7], words[6], words[5], words[4],
                                 words[3], words[2], words[1], words[0]);
    }

    for (int i = 16; i < 64; i++) {
        W[i] = _mm512_add_epi32(_mm512_add_epi32(gamma1_avx512(W[i-2]), W[i-7]),
                                _mm512_add_epi32(gamma0_avx512(W[i-15]), W[i-16]));
    }

    // 64 rounds
    for (int i = 0; i < 64; i++) {
        __m512i T1 = _mm512_add_epi32(
            _mm512_add_epi32(h, sigma1_avx512(e)),
            _mm512_add_epi32(_mm512_add_epi32(ch_avx512(e, f, g), K_avx512(i)), W[i]));
        __m512i T2 = _mm512_add_epi32(sigma0_avx512(a), maj_avx512(a, b, c));

        h = g; g = f; f = e;
        e = _mm512_add_epi32(d, T1);
        d = c; c = b; b = a;
        a = _mm512_add_epi32(T1, T2);
    }

    // Add to saved state
    a = _mm512_add_epi32(a, a_save);
    b = _mm512_add_epi32(b, b_save);
    c = _mm512_add_epi32(c, c_save);
    d = _mm512_add_epi32(d, d_save);
    e = _mm512_add_epi32(e, e_save);
    f = _mm512_add_epi32(f, f_save);
    g = _mm512_add_epi32(g, g_save);
    h = _mm512_add_epi32(h, h_save);

    // Store back
    alignas(64) uint32_t temp[16];

    _mm512_store_si512((__m512i*)temp, a);
    for (int lane = 0; lane < 16; lane++) state[0][lane] = temp[lane];

    _mm512_store_si512((__m512i*)temp, b);
    for (int lane = 0; lane < 16; lane++) state[1][lane] = temp[lane];

    _mm512_store_si512((__m512i*)temp, c);
    for (int lane = 0; lane < 16; lane++) state[2][lane] = temp[lane];

    _mm512_store_si512((__m512i*)temp, d);
    for (int lane = 0; lane < 16; lane++) state[3][lane] = temp[lane];

    _mm512_store_si512((__m512i*)temp, e);
    for (int lane = 0; lane < 16; lane++) state[4][lane] = temp[lane];

    _mm512_store_si512((__m512i*)temp, f);
    for (int lane = 0; lane < 16; lane++) state[5][lane] = temp[lane];

    _mm512_store_si512((__m512i*)temp, g);
    for (int lane = 0; lane < 16; lane++) state[6][lane] = temp[lane];

    _mm512_store_si512((__m512i*)temp, h);
    for (int lane = 0; lane < 16; lane++) state[7][lane] = temp[lane];
}

// Process 16 nonces with midstate optimization
inline uint16_t sha256d_midstate_16way(const Midstate& ms, uint32_t nonce_base,
                                        const uint8_t target[32], uint8_t hashes[16][32]) {
    alignas(64) uint32_t state[8][16];
    alignas(64) uint8_t blocks[16][64];
    alignas(64) uint8_t first_hash[16][32];

    // Initialize state from midstate
    for (int lane = 0; lane < 16; lane++) {
        for (int i = 0; i < 8; i++) {
            state[i][lane] = ms.state[i];
        }
    }

    // Build 16 blocks with consecutive nonces
    for (int lane = 0; lane < 16; lane++) {
        std::memset(blocks[lane], 0, 64);
        std::memcpy(blocks[lane], ms.tail, 12);

        uint32_t nonce = nonce_base + lane;
        blocks[lane][12] = nonce & 0xff;
        blocks[lane][13] = (nonce >> 8) & 0xff;
        blocks[lane][14] = (nonce >> 16) & 0xff;
        blocks[lane][15] = (nonce >> 24) & 0xff;

        blocks[lane][16] = 0x80;
        blocks[lane][62] = 0x02;
        blocks[lane][63] = 0x80;
    }

    // First transform
    transform_avx512_16way(state, blocks);

    // Convert to bytes
    for (int lane = 0; lane < 16; lane++) {
        for (int i = 0; i < 8; i++) {
            first_hash[lane][i*4 + 0] = (state[i][lane] >> 24) & 0xff;
            first_hash[lane][i*4 + 1] = (state[i][lane] >> 16) & 0xff;
            first_hash[lane][i*4 + 2] = (state[i][lane] >> 8) & 0xff;
            first_hash[lane][i*4 + 3] = state[i][lane] & 0xff;
        }
    }

    // Reset state for second hash
    for (int lane = 0; lane < 16; lane++) {
        for (int i = 0; i < 8; i++) {
            state[i][lane] = H_INIT[i];
        }
    }

    // Build second blocks
    for (int lane = 0; lane < 16; lane++) {
        std::memset(blocks[lane], 0, 64);
        std::memcpy(blocks[lane], first_hash[lane], 32);
        blocks[lane][32] = 0x80;
        blocks[lane][62] = 0x01;
        blocks[lane][63] = 0x00;
    }

    // Second transform
    transform_avx512_16way(state, blocks);

    // Convert and check target
    uint16_t valid_mask = 0;

    for (int lane = 0; lane < 16; lane++) {
        for (int i = 0; i < 8; i++) {
            hashes[lane][i*4 + 3] = (state[i][lane] >> 24) & 0xff;
            hashes[lane][i*4 + 2] = (state[i][lane] >> 16) & 0xff;
            hashes[lane][i*4 + 1] = (state[i][lane] >> 8) & 0xff;
            hashes[lane][i*4 + 0] = state[i][lane] & 0xff;
        }

        bool valid = true;
        for (int i = 31; i >= 0 && valid; i--) {
            if (hashes[lane][i] < target[i]) break;
            if (hashes[lane][i] > target[i]) valid = false;
        }
        if (valid) valid_mask |= (1 << lane);
    }

    return valid_mask;
}

#endif // HAVE_AVX512

// Feature detection functions
inline bool has_avx2() {
#ifdef HAVE_AVX2
    return true;
#else
    return false;
#endif
}

inline bool has_avx512() {
#ifdef HAVE_AVX512
    return true;
#else
    return false;
#endif
}

// Get best batch size for current CPU
inline unsigned get_optimal_batch_size() {
#ifdef HAVE_AVX512
    return 16;
#elif defined(HAVE_AVX2)
    return 8;
#else
    return 1;  // No SIMD batching
#endif
}

} // namespace sha256
