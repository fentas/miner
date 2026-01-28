#pragma once

#include <cstdint>
#include <cstring>
#include <array>

#ifdef HAVE_SHA_NI
#include <immintrin.h>
#endif

namespace sha256 {

// SHA-256 initial hash values
constexpr std::array<uint32_t, 8> H_INIT = {
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
};

// SHA-256 round constants
constexpr std::array<uint32_t, 64> K = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
    0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
    0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
    0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
    0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
    0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
    0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
    0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
    0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
};

inline uint32_t rotr(uint32_t x, int n) {
    return (x >> n) | (x << (32 - n));
}

inline uint32_t ch(uint32_t x, uint32_t y, uint32_t z) {
    return (x & y) ^ (~x & z);
}

inline uint32_t maj(uint32_t x, uint32_t y, uint32_t z) {
    return (x & y) ^ (x & z) ^ (y & z);
}

inline uint32_t sigma0(uint32_t x) {
    return rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22);
}

inline uint32_t sigma1(uint32_t x) {
    return rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25);
}

inline uint32_t gamma0(uint32_t x) {
    return rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3);
}

inline uint32_t gamma1(uint32_t x) {
    return rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10);
}

inline uint32_t bswap32(uint32_t x) {
    return __builtin_bswap32(x);
}

// Generic SHA-256 single block transform
inline void transform_generic(uint32_t state[8], const uint8_t block[64]) {
    uint32_t W[64];
    uint32_t a, b, c, d, e, f, g, h;

    // Prepare message schedule
    for (int i = 0; i < 16; i++) {
        W[i] = bswap32(reinterpret_cast<const uint32_t*>(block)[i]);
    }
    for (int i = 16; i < 64; i++) {
        W[i] = gamma1(W[i-2]) + W[i-7] + gamma0(W[i-15]) + W[i-16];
    }

    // Initialize working variables
    a = state[0]; b = state[1]; c = state[2]; d = state[3];
    e = state[4]; f = state[5]; g = state[6]; h = state[7];

    // 64 rounds
    for (int i = 0; i < 64; i++) {
        uint32_t T1 = h + sigma1(e) + ch(e, f, g) + K[i] + W[i];
        uint32_t T2 = sigma0(a) + maj(a, b, c);
        h = g; g = f; f = e; e = d + T1;
        d = c; c = b; b = a; a = T1 + T2;
    }

    // Add to state
    state[0] += a; state[1] += b; state[2] += c; state[3] += d;
    state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

#ifdef HAVE_SHA_NI
// SHA-NI accelerated transform (Intel SHA Extensions)
inline void transform_shani(uint32_t state[8], const uint8_t block[64]) {
    __m128i STATE0, STATE1;
    __m128i MSG, TMP;
    __m128i MSG0, MSG1, MSG2, MSG3;
    __m128i ABEF_SAVE, CDGH_SAVE;
    const __m128i MASK = _mm_set_epi64x(0x0c0d0e0f08090a0bULL, 0x0405060700010203ULL);

    // Load initial state
    TMP = _mm_loadu_si128(reinterpret_cast<const __m128i*>(&state[0]));
    STATE1 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(&state[4]));

    TMP = _mm_shuffle_epi32(TMP, 0xB1);          // CDAB
    STATE1 = _mm_shuffle_epi32(STATE1, 0x1B);    // EFGH
    STATE0 = _mm_alignr_epi8(TMP, STATE1, 8);    // ABEF
    STATE1 = _mm_blend_epi16(STATE1, TMP, 0xF0); // CDGH

    // Save current state
    ABEF_SAVE = STATE0;
    CDGH_SAVE = STATE1;

    // Rounds 0-3
    MSG = _mm_loadu_si128(reinterpret_cast<const __m128i*>(block + 0));
    MSG0 = _mm_shuffle_epi8(MSG, MASK);
    MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0xE9B5DBA5B5C0FBCFULL, 0x71374491428A2F98ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

    // Rounds 4-7
    MSG1 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(block + 16));
    MSG1 = _mm_shuffle_epi8(MSG1, MASK);
    MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0xAB1C5ED5923F82A4ULL, 0x59F111F13956C25BULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

    // Rounds 8-11
    MSG2 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(block + 32));
    MSG2 = _mm_shuffle_epi8(MSG2, MASK);
    MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0x550C7DC3243185BEULL, 0x12835B01D807AA98ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

    // Rounds 12-15
    MSG3 = _mm_loadu_si128(reinterpret_cast<const __m128i*>(block + 48));
    MSG3 = _mm_shuffle_epi8(MSG3, MASK);
    MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0xC19BF1749BDC06A7ULL, 0x80DEB1FE72BE5D74ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
    MSG0 = _mm_add_epi32(MSG0, TMP);
    MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

    // Rounds 16-19
    MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0x240CA1CC0FC19DC6ULL, 0xEFBE4786E49B69C1ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
    MSG1 = _mm_add_epi32(MSG1, TMP);
    MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

    // Rounds 20-23
    MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0x76F988DA5CB0A9DCULL, 0x4A7484AA2DE92C6FULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
    MSG2 = _mm_add_epi32(MSG2, TMP);
    MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

    // Rounds 24-27
    MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0xBF597FC7B00327C8ULL, 0xA831C66D983E5152ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
    MSG3 = _mm_add_epi32(MSG3, TMP);
    MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

    // Rounds 28-31
    MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0x1429296706CA6351ULL, 0xD5A79147C6E00BF3ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
    MSG0 = _mm_add_epi32(MSG0, TMP);
    MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

    // Rounds 32-35
    MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0x53380D134D2C6DFCULL, 0x2E1B213827B70A85ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
    MSG1 = _mm_add_epi32(MSG1, TMP);
    MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

    // Rounds 36-39
    MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0x92722C8581C2C92EULL, 0x766A0ABB650A7354ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
    MSG2 = _mm_add_epi32(MSG2, TMP);
    MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG0 = _mm_sha256msg1_epu32(MSG0, MSG1);

    // Rounds 40-43
    MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0xC76C51A3C24B8B70ULL, 0xA81A664BA2BFE8A1ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
    MSG3 = _mm_add_epi32(MSG3, TMP);
    MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG1 = _mm_sha256msg1_epu32(MSG1, MSG2);

    // Rounds 44-47
    MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0x106AA070F40E3585ULL, 0xD6990624D192E819ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG3, MSG2, 4);
    MSG0 = _mm_add_epi32(MSG0, TMP);
    MSG0 = _mm_sha256msg2_epu32(MSG0, MSG3);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG2 = _mm_sha256msg1_epu32(MSG2, MSG3);

    // Rounds 48-51
    MSG = _mm_add_epi32(MSG0, _mm_set_epi64x(0x34B0BCB52748774CULL, 0x1E376C0819A4C116ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG0, MSG3, 4);
    MSG1 = _mm_add_epi32(MSG1, TMP);
    MSG1 = _mm_sha256msg2_epu32(MSG1, MSG0);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);
    MSG3 = _mm_sha256msg1_epu32(MSG3, MSG0);

    // Rounds 52-55
    MSG = _mm_add_epi32(MSG1, _mm_set_epi64x(0x682E6FF35B9CCA4FULL, 0x4ED8AA4A391C0CB3ULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG1, MSG0, 4);
    MSG2 = _mm_add_epi32(MSG2, TMP);
    MSG2 = _mm_sha256msg2_epu32(MSG2, MSG1);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

    // Rounds 56-59
    MSG = _mm_add_epi32(MSG2, _mm_set_epi64x(0x8CC7020884C87814ULL, 0x78A5636F748F82EEULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    TMP = _mm_alignr_epi8(MSG2, MSG1, 4);
    MSG3 = _mm_add_epi32(MSG3, TMP);
    MSG3 = _mm_sha256msg2_epu32(MSG3, MSG2);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

    // Rounds 60-63
    MSG = _mm_add_epi32(MSG3, _mm_set_epi64x(0xC67178F2BEF9A3F7ULL, 0xA4506CEB90BEFFFAULL));
    STATE1 = _mm_sha256rnds2_epu32(STATE1, STATE0, MSG);
    MSG = _mm_shuffle_epi32(MSG, 0x0E);
    STATE0 = _mm_sha256rnds2_epu32(STATE0, STATE1, MSG);

    // Add saved state
    STATE0 = _mm_add_epi32(STATE0, ABEF_SAVE);
    STATE1 = _mm_add_epi32(STATE1, CDGH_SAVE);

    // Reorder and store state
    TMP = _mm_shuffle_epi32(STATE0, 0x1B);       // FEBA
    STATE1 = _mm_shuffle_epi32(STATE1, 0xB1);    // DCHG
    STATE0 = _mm_blend_epi16(TMP, STATE1, 0xF0); // DCBA
    STATE1 = _mm_alignr_epi8(STATE1, TMP, 8);    // ABEF -> HGFE

    _mm_storeu_si128(reinterpret_cast<__m128i*>(&state[0]), STATE0);
    _mm_storeu_si128(reinterpret_cast<__m128i*>(&state[4]), STATE1);
}
#endif

// Choose best implementation at compile time
inline void transform(uint32_t state[8], const uint8_t block[64]) {
#ifdef HAVE_SHA_NI
    transform_shani(state, block);
#else
    transform_generic(state, block);
#endif
}

// SHA-256 double hash for Bitcoin (80-byte block header -> 32-byte hash)
inline void sha256d_80(const uint8_t header[80], uint8_t hash[32]) {
    uint32_t state[8];
    uint8_t padded[128];

    // First SHA-256: hash the 80-byte header
    std::memcpy(state, H_INIT.data(), 32);

    // First block (64 bytes)
    transform(state, header);

    // Second block (16 bytes data + padding)
    std::memset(padded, 0, 64);
    std::memcpy(padded, header + 64, 16);
    padded[16] = 0x80;  // Padding bit
    // Length in bits = 80 * 8 = 640 = 0x280
    padded[62] = 0x02;
    padded[63] = 0x80;
    transform(state, padded);

    // Convert first hash to bytes (big-endian)
    uint8_t first_hash[32];
    for (int i = 0; i < 8; i++) {
        first_hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        first_hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        first_hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        first_hash[i*4 + 3] = state[i] & 0xff;
    }

    // Second SHA-256: hash the 32-byte first hash
    std::memcpy(state, H_INIT.data(), 32);
    std::memset(padded, 0, 64);
    std::memcpy(padded, first_hash, 32);
    padded[32] = 0x80;  // Padding bit
    // Length in bits = 32 * 8 = 256 = 0x100
    padded[62] = 0x01;
    padded[63] = 0x00;
    transform(state, padded);

    // Output hash (little-endian for Bitcoin comparison)
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 3] = (state[i] >> 24) & 0xff;
        hash[i*4 + 2] = (state[i] >> 16) & 0xff;
        hash[i*4 + 1] = (state[i] >> 8) & 0xff;
        hash[i*4 + 0] = state[i] & 0xff;
    }
}

// Optimized: pre-compute midstate for first 64 bytes, only hash last 16 bytes + nonce
struct Midstate {
    uint32_t state[8];
    uint8_t tail[16];  // Last 16 bytes of header (before nonce)
};

inline void compute_midstate(const uint8_t header[80], Midstate& ms) {
    std::memcpy(ms.state, H_INIT.data(), 32);
    transform(ms.state, header);
    std::memcpy(ms.tail, header + 64, 16);
}

// Hash with pre-computed midstate and nonce
inline void sha256d_midstate(const Midstate& ms, uint32_t nonce, uint8_t hash[32]) {
    uint32_t state[8];
    uint8_t block[64];

    // Copy midstate
    std::memcpy(state, ms.state, 32);

    // Build second block: tail (12 bytes) + nonce (4 bytes) + padding
    std::memset(block, 0, 64);
    std::memcpy(block, ms.tail, 12);

    // Nonce at offset 12 (little-endian)
    block[12] = nonce & 0xff;
    block[13] = (nonce >> 8) & 0xff;
    block[14] = (nonce >> 16) & 0xff;
    block[15] = (nonce >> 24) & 0xff;

    block[16] = 0x80;  // Padding
    block[62] = 0x02;  // Length high byte (640 bits = 0x280)
    block[63] = 0x80;  // Length low byte

    transform(state, block);

    // First hash complete, now second SHA-256
    uint8_t first_hash[32];
    for (int i = 0; i < 8; i++) {
        first_hash[i*4 + 0] = (state[i] >> 24) & 0xff;
        first_hash[i*4 + 1] = (state[i] >> 16) & 0xff;
        first_hash[i*4 + 2] = (state[i] >> 8) & 0xff;
        first_hash[i*4 + 3] = state[i] & 0xff;
    }

    std::memcpy(state, H_INIT.data(), 32);
    std::memset(block, 0, 64);
    std::memcpy(block, first_hash, 32);
    block[32] = 0x80;
    block[62] = 0x01;
    block[63] = 0x00;
    transform(state, block);

    // Output (little-endian)
    for (int i = 0; i < 8; i++) {
        hash[i*4 + 3] = (state[i] >> 24) & 0xff;
        hash[i*4 + 2] = (state[i] >> 16) & 0xff;
        hash[i*4 + 1] = (state[i] >> 8) & 0xff;
        hash[i*4 + 0] = state[i] & 0xff;
    }
}

} // namespace sha256
