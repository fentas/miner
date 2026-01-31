//! Generic portable SHA-256 implementation
//!
//! Fallback implementation that works on all platforms.

use super::{H_INIT, K, Midstate, sha256_pad, state_to_bytes, hash_meets_target};

/// Generic SHA-256 single block transform
pub fn transform(state: &mut [u32; 8], block: &[u8]) {
    let mut w = [0u32; 64];

    // Prepare message schedule
    for i in 0..16 {
        w[i] = u32::from_be_bytes([
            block[i * 4],
            block[i * 4 + 1],
            block[i * 4 + 2],
            block[i * 4 + 3],
        ]);
    }

    for i in 16..64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];
    let mut f = state[5];
    let mut g = state[6];
    let mut h = state[7];

    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = h
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        h = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

/// Double SHA-256 using generic implementation
pub fn sha256d(data: &[u8]) -> [u8; 32] {
    // First hash
    let mut state = H_INIT;
    let padded = sha256_pad(data);
    for chunk in padded.chunks(64) {
        transform(&mut state, chunk);
    }
    let first_hash = state_to_bytes(&state);

    // Second hash
    let mut state = H_INIT;
    let padded = sha256_pad(&first_hash);
    for chunk in padded.chunks(64) {
        transform(&mut state, chunk);
    }
    state_to_bytes(&state)
}

/// SHA-256 with midstate (for mining - only last 16 bytes change)
///
/// The tail parameter contains bytes 64-75 of the header (12 bytes, padded to 16).
/// The nonce is placed at bytes 76-79 (block[12..16]) to match an 80-byte header.
pub fn sha256d_midstate(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    // Build the last block for an 80-byte message:
    // - bytes 64-75: tail[0..12] (12 bytes of header data)
    // - bytes 76-79: nonce (4 bytes)
    // - byte 80: 0x80 padding marker
    // - bytes 81-125: zeros
    // - bytes 126-127: length in bits (640 = 0x0280)
    let mut block = [0u8; 64];
    block[..12].copy_from_slice(&tail[..12]);
    block[12..16].copy_from_slice(&nonce.to_le_bytes());
    // Padding for 80-byte message
    block[16] = 0x80;
    // Length in bits (80 * 8 = 640 = 0x280) at end
    block[62] = 0x02;
    block[63] = 0x80;

    // First hash (continuing from midstate)
    let mut state = midstate.state;
    transform(&mut state, &block);
    let first_hash = state_to_bytes(&state);

    // Second hash
    let mut state = H_INIT;
    let mut block2 = [0u8; 64];
    block2[..32].copy_from_slice(&first_hash);
    block2[32] = 0x80;
    block2[62] = 0x01; // 256 bits = 0x100
    block2[63] = 0x00;
    transform(&mut state, &block2);
    state_to_bytes(&state)
}

/// Generic mining loop
pub fn mine(
    midstate: &Midstate,
    tail: &[u8; 16],
    nonce_start: u32,
    nonce_count: u32,
    target: &[u8; 32],
) -> Option<(u32, [u8; 32])> {
    for i in 0..nonce_count {
        let nonce = nonce_start.wrapping_add(i);
        let hash = sha256d_midstate(midstate, tail, nonce);
        if hash_meets_target(&hash, target) {
            return Some((nonce, hash));
        }
    }
    None
}
