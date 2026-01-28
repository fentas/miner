// OpenCL SHA-256d Kernel for Bitcoin Mining

// SHA-256 round constants
__constant uint K[64] = {
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

__constant uint H_INIT[8] = {
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
    0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
};

// Rotate right
inline uint rotr(uint x, int n) {
    return rotate(x, (uint)(32 - n));
}

// SHA-256 functions
inline uint ch(uint x, uint y, uint z) {
    return (x & y) ^ (~x & z);
}

inline uint maj(uint x, uint y, uint z) {
    return (x & y) ^ (x & z) ^ (y & z);
}

inline uint sigma0(uint x) {
    return rotr(x, 2) ^ rotr(x, 13) ^ rotr(x, 22);
}

inline uint sigma1(uint x) {
    return rotr(x, 6) ^ rotr(x, 11) ^ rotr(x, 25);
}

inline uint gamma0(uint x) {
    return rotr(x, 7) ^ rotr(x, 18) ^ (x >> 3);
}

inline uint gamma1(uint x) {
    return rotr(x, 17) ^ rotr(x, 19) ^ (x >> 10);
}

// Byte swap
inline uint bswap32(uint x) {
    return ((x & 0xff) << 24) | ((x & 0xff00) << 8) |
           ((x >> 8) & 0xff00) | ((x >> 24) & 0xff);
}

// SHA-256 transform
void sha256_transform(uint state[8], uint block[16]) {
    uint W[64];
    uint a, b, c, d, e, f, g, h;

    // Prepare message schedule
    for (int i = 0; i < 16; i++) {
        W[i] = block[i];
    }
    for (int i = 16; i < 64; i++) {
        W[i] = gamma1(W[i-2]) + W[i-7] + gamma0(W[i-15]) + W[i-16];
    }

    // Initialize working variables
    a = state[0]; b = state[1]; c = state[2]; d = state[3];
    e = state[4]; f = state[5]; g = state[6]; h = state[7];

    // 64 rounds
    for (int i = 0; i < 64; i++) {
        uint T1 = h + sigma1(e) + ch(e, f, g) + K[i] + W[i];
        uint T2 = sigma0(a) + maj(a, b, c);
        h = g; g = f; f = e; e = d + T1;
        d = c; c = b; b = a; a = T1 + T2;
    }

    // Add to state
    state[0] += a; state[1] += b; state[2] += c; state[3] += d;
    state[4] += e; state[5] += f; state[6] += g; state[7] += h;
}

// Check if hash meets target
bool check_target(uint hash[8], __global const uchar* target) {
    for (int i = 7; i >= 0; i--) {
        uint target_word = ((uint)target[i*4 + 3] << 24) |
                           ((uint)target[i*4 + 2] << 16) |
                           ((uint)target[i*4 + 1] << 8) |
                           ((uint)target[i*4 + 0]);
        if (hash[i] < target_word) return true;
        if (hash[i] > target_word) return false;
    }
    return true;
}

// Main mining kernel
__kernel void sha256d_kernel(
    __global const uint* midstate,
    __global const uchar* tail,
    __global const uchar* target,
    uint nonce_start,
    __global uint* result_nonces,
    __global uchar* result_hashes,
    __global uint* result_count,
    int max_results
) {
    uint nonce = nonce_start + get_global_id(0);

    uint state[8];
    uint block[16];

    // Copy midstate
    for (int i = 0; i < 8; i++) {
        state[i] = midstate[i];
    }

    // Clear block
    for (int i = 0; i < 16; i++) {
        block[i] = 0;
    }

    // Load tail (12 bytes)
    block[0] = bswap32(((uint)tail[0]) | ((uint)tail[1] << 8) |
                        ((uint)tail[2] << 16) | ((uint)tail[3] << 24));
    block[1] = bswap32(((uint)tail[4]) | ((uint)tail[5] << 8) |
                        ((uint)tail[6] << 16) | ((uint)tail[7] << 24));
    block[2] = bswap32(((uint)tail[8]) | ((uint)tail[9] << 8) |
                        ((uint)tail[10] << 16) | ((uint)tail[11] << 24));

    // Nonce
    block[3] = bswap32(nonce);

    // Padding
    block[4] = 0x80000000;
    block[15] = 80 * 8;

    // First SHA-256
    sha256_transform(state, block);

    // Second SHA-256
    uint first_hash[8];
    for (int i = 0; i < 8; i++) {
        first_hash[i] = state[i];
        state[i] = H_INIT[i];
    }

    for (int i = 0; i < 8; i++) {
        block[i] = bswap32(first_hash[i]);
    }
    block[8] = 0x80000000;
    for (int i = 9; i < 15; i++) {
        block[i] = 0;
    }
    block[15] = 32 * 8;

    sha256_transform(state, block);

    // Check target
    if (check_target(state, target)) {
        int idx = atomic_add(result_count, 1);
        if (idx < max_results) {
            result_nonces[idx] = nonce;

            for (int i = 0; i < 8; i++) {
                result_hashes[idx * 32 + i*4 + 0] = state[i] & 0xff;
                result_hashes[idx * 32 + i*4 + 1] = (state[i] >> 8) & 0xff;
                result_hashes[idx * 32 + i*4 + 2] = (state[i] >> 16) & 0xff;
                result_hashes[idx * 32 + i*4 + 3] = (state[i] >> 24) & 0xff;
            }
        }
    }
}
