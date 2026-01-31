//! SHA-256 using inline assembly (x86_64 SHA-NI)
//!
//! This implementation uses the `asm!` macro to avoid the #[target_feature]
//! inlining boundary. The entire SHA-256 transform is expressed in inline
//! assembly, allowing the compiler to inline the hot loop.
//!
//! Key advantage: No `#[target_feature]` on public functions, so the calling
//! code (the mining loop) can be fully inlined by the compiler.

use super::{H_INIT, K, Midstate, state_to_bytes};

/// SHA-256 K constants packed for SIMD loading (little-endian byte order for memory load)
/// Each row is [K[i], K[i+1], K[i+2], K[i+3]] as it will be loaded by movdqu
#[repr(align(16))]
struct AlignedK([[u32; 4]; 16]);

static K_PACKED: AlignedK = AlignedK([
    [K[0], K[1], K[2], K[3]],
    [K[4], K[5], K[6], K[7]],
    [K[8], K[9], K[10], K[11]],
    [K[12], K[13], K[14], K[15]],
    [K[16], K[17], K[18], K[19]],
    [K[20], K[21], K[22], K[23]],
    [K[24], K[25], K[26], K[27]],
    [K[28], K[29], K[30], K[31]],
    [K[32], K[33], K[34], K[35]],
    [K[36], K[37], K[38], K[39]],
    [K[40], K[41], K[42], K[43]],
    [K[44], K[45], K[46], K[47]],
    [K[48], K[49], K[50], K[51]],
    [K[52], K[53], K[54], K[55]],
    [K[56], K[57], K[58], K[59]],
    [K[60], K[61], K[62], K[63]],
]);

/// Byte-swap mask for converting little-endian to big-endian (for SSSE3 pshufb)
#[repr(align(16))]
struct AlignedMask([u8; 16]);

static BSWAP_MASK: AlignedMask = AlignedMask([
    3, 2, 1, 0,    // Word 0: swap bytes
    7, 6, 5, 4,    // Word 1: swap bytes
    11, 10, 9, 8,  // Word 2: swap bytes
    15, 14, 13, 12 // Word 3: swap bytes
]);

/// SHA-256 transform using inline assembly with SHA-NI instructions
///
/// This function does NOT have #[target_feature] so it CAN be inlined.
/// Caller must ensure SHA-NI is available before calling.
#[inline(always)]
unsafe fn transform_asm(state: &mut [u32; 8], block: &[u8; 64]) {
    use std::arch::asm;

    let k_ptr = K_PACKED.0.as_ptr();
    let bswap_ptr = BSWAP_MASK.0.as_ptr();

    // SHA-NI state layout:
    // state0 (ABEF): Contains A, B, E, F in that order
    // state1 (CDGH): Contains C, D, G, H in that order
    //
    // sha256rnds2 implicitly uses xmm0 as the message+K source

    asm!(
        // =====================================================
        // PROLOGUE: Load initial state into XMM registers
        // =====================================================

        // Load state words into XMM registers
        // tmp = [A, B, C, D] from state[0..4] (little-endian dwords)
        // state1 = [E, F, G, H] from state[4..8]
        "movdqu {tmp}, [{state_ptr}]",
        "movdqu {state1}, [{state_ptr} + 16]",

        // Shuffle to Intel SHA-NI format:
        // pshufd tmp, tmp, 0xB1 -> [B, A, D, C]
        // pshufd state1, state1, 0x1B -> [H, G, F, E]
        "pshufd {tmp}, {tmp}, 0xB1",
        "pshufd {state1}, {state1}, 0x1B",

        // state0 = palignr(tmp, state1, 8) -> [A, B, E, F] (ABEF)
        "movdqa {state0}, {tmp}",
        "palignr {state0}, {state1}, 8",

        // state1 = pblendw(state1, tmp, 0xF0) -> [C, D, G, H] (CDGH)
        "pblendw {state1}, {tmp}, 0xF0",

        // Save initial state for final addition
        "movdqa {abef_save}, {state0}",
        "movdqa {cdgh_save}, {state1}",

        // =====================================================
        // MESSAGE SCHEDULE: Load and byte-swap message blocks
        // =====================================================

        // Load byte-swap mask for little->big endian conversion
        "movdqa {shuf_mask}, [{bswap_ptr}]",

        // Load message words and byte-swap
        "movdqu {msg0}, [{block_ptr}]",
        "pshufb {msg0}, {shuf_mask}",
        "movdqu {msg1}, [{block_ptr} + 16]",
        "pshufb {msg1}, {shuf_mask}",
        "movdqu {msg2}, [{block_ptr} + 32]",
        "pshufb {msg2}, {shuf_mask}",
        "movdqu {msg3}, [{block_ptr} + 48]",
        "pshufb {msg3}, {shuf_mask}",

        // =====================================================
        // ROUNDS 0-3
        // =====================================================
        "movdqa xmm0, {msg0}",
        "paddd xmm0, [{k_ptr}]",
        "sha256rnds2 {state1}, {state0}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",

        // =====================================================
        // ROUNDS 4-7
        // =====================================================
        "movdqa xmm0, {msg1}",
        "paddd xmm0, [{k_ptr} + 16]",
        "sha256rnds2 {state1}, {state0}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg0}, {msg1}",

        // =====================================================
        // ROUNDS 8-11
        // =====================================================
        "movdqa xmm0, {msg2}",
        "paddd xmm0, [{k_ptr} + 32]",
        "sha256rnds2 {state1}, {state0}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg1}, {msg2}",

        // =====================================================
        // ROUNDS 12-15
        // =====================================================
        "movdqa xmm0, {msg3}",
        "paddd xmm0, [{k_ptr} + 48]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg3}",
        "palignr {tmp}, {msg2}, 4",
        "paddd {msg0}, {tmp}",
        "sha256msg2 {msg0}, {msg3}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg2}, {msg3}",

        // =====================================================
        // ROUNDS 16-19
        // =====================================================
        "movdqa xmm0, {msg0}",
        "paddd xmm0, [{k_ptr} + 64]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg0}",
        "palignr {tmp}, {msg3}, 4",
        "paddd {msg1}, {tmp}",
        "sha256msg2 {msg1}, {msg0}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg3}, {msg0}",

        // =====================================================
        // ROUNDS 20-23
        // =====================================================
        "movdqa xmm0, {msg1}",
        "paddd xmm0, [{k_ptr} + 80]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg1}",
        "palignr {tmp}, {msg0}, 4",
        "paddd {msg2}, {tmp}",
        "sha256msg2 {msg2}, {msg1}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg0}, {msg1}",

        // =====================================================
        // ROUNDS 24-27
        // =====================================================
        "movdqa xmm0, {msg2}",
        "paddd xmm0, [{k_ptr} + 96]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg2}",
        "palignr {tmp}, {msg1}, 4",
        "paddd {msg3}, {tmp}",
        "sha256msg2 {msg3}, {msg2}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg1}, {msg2}",

        // =====================================================
        // ROUNDS 28-31
        // =====================================================
        "movdqa xmm0, {msg3}",
        "paddd xmm0, [{k_ptr} + 112]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg3}",
        "palignr {tmp}, {msg2}, 4",
        "paddd {msg0}, {tmp}",
        "sha256msg2 {msg0}, {msg3}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg2}, {msg3}",

        // =====================================================
        // ROUNDS 32-35
        // =====================================================
        "movdqa xmm0, {msg0}",
        "paddd xmm0, [{k_ptr} + 128]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg0}",
        "palignr {tmp}, {msg3}, 4",
        "paddd {msg1}, {tmp}",
        "sha256msg2 {msg1}, {msg0}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg3}, {msg0}",

        // =====================================================
        // ROUNDS 36-39
        // =====================================================
        "movdqa xmm0, {msg1}",
        "paddd xmm0, [{k_ptr} + 144]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg1}",
        "palignr {tmp}, {msg0}, 4",
        "paddd {msg2}, {tmp}",
        "sha256msg2 {msg2}, {msg1}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg0}, {msg1}",

        // =====================================================
        // ROUNDS 40-43
        // =====================================================
        "movdqa xmm0, {msg2}",
        "paddd xmm0, [{k_ptr} + 160]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg2}",
        "palignr {tmp}, {msg1}, 4",
        "paddd {msg3}, {tmp}",
        "sha256msg2 {msg3}, {msg2}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg1}, {msg2}",

        // =====================================================
        // ROUNDS 44-47
        // =====================================================
        "movdqa xmm0, {msg3}",
        "paddd xmm0, [{k_ptr} + 176]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg3}",
        "palignr {tmp}, {msg2}, 4",
        "paddd {msg0}, {tmp}",
        "sha256msg2 {msg0}, {msg3}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg2}, {msg3}",

        // =====================================================
        // ROUNDS 48-51
        // =====================================================
        "movdqa xmm0, {msg0}",
        "paddd xmm0, [{k_ptr} + 192]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg0}",
        "palignr {tmp}, {msg3}, 4",
        "paddd {msg1}, {tmp}",
        "sha256msg2 {msg1}, {msg0}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",
        "sha256msg1 {msg3}, {msg0}",

        // =====================================================
        // ROUNDS 52-55 (no further msg expansion after msg1)
        // =====================================================
        "movdqa xmm0, {msg1}",
        "paddd xmm0, [{k_ptr} + 208]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg1}",
        "palignr {tmp}, {msg0}, 4",
        "paddd {msg2}, {tmp}",
        "sha256msg2 {msg2}, {msg1}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",

        // =====================================================
        // ROUNDS 56-59 (no further msg expansion)
        // =====================================================
        "movdqa xmm0, {msg2}",
        "paddd xmm0, [{k_ptr} + 224]",
        "sha256rnds2 {state1}, {state0}",
        "movdqa {tmp}, {msg2}",
        "palignr {tmp}, {msg1}, 4",
        "paddd {msg3}, {tmp}",
        "sha256msg2 {msg3}, {msg2}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",

        // =====================================================
        // ROUNDS 60-63 (final rounds, no message expansion)
        // =====================================================
        "movdqa xmm0, {msg3}",
        "paddd xmm0, [{k_ptr} + 240]",
        "sha256rnds2 {state1}, {state0}",
        "pshufd xmm0, xmm0, 0x0E",
        "sha256rnds2 {state0}, {state1}",

        // =====================================================
        // EPILOGUE: Add saved state and store result
        // =====================================================

        // Add initial state back
        "paddd {state0}, {abef_save}",
        "paddd {state1}, {cdgh_save}",

        // Convert back from ABEF/CDGH to [A,B,C,D,E,F,G,H] format
        // tmp = pshufd(state0, 0x1B) -> [F, E, B, A]
        "pshufd {tmp}, {state0}, 0x1B",
        // state1 = pshufd(state1, 0xB1) -> [D, C, H, G]
        "pshufd {state1}, {state1}, 0xB1",
        // msg0 = pblendw(tmp, state1, 0xF0) -> [D, C, B, A] (state[0..4])
        "movdqa {msg0}, {tmp}",
        "pblendw {msg0}, {state1}, 0xF0",
        // state1 = palignr(state1, tmp, 8) -> [H, G, F, E] (state[4..8])
        "palignr {state1}, {tmp}, 8",

        // Store back to state
        "movdqu [{state_ptr}], {msg0}",
        "movdqu [{state_ptr} + 16], {state1}",

        // Input/output operands
        state_ptr = in(reg) state.as_mut_ptr(),
        block_ptr = in(reg) block.as_ptr(),
        k_ptr = in(reg) k_ptr,
        bswap_ptr = in(reg) bswap_ptr,

        // XMM register operands
        // Note: sha256rnds2 implicitly uses xmm0, so we explicitly use it for msg
        state0 = out(xmm_reg) _,
        state1 = out(xmm_reg) _,
        msg0 = out(xmm_reg) _,
        msg1 = out(xmm_reg) _,
        msg2 = out(xmm_reg) _,
        msg3 = out(xmm_reg) _,
        tmp = out(xmm_reg) _,
        shuf_mask = out(xmm_reg) _,
        abef_save = out(xmm_reg) _,
        cdgh_save = out(xmm_reg) _,

        // xmm0 is clobbered by explicit use above
        out("xmm0") _,

        // Options
        options(nostack, preserves_flags),
    );
}

/// Double SHA-256 with midstate using inline assembly
///
/// This function has NO #[target_feature] attribute, so it CAN be inlined
/// into the hot mining loop. Runtime detection happens inside.
///
/// # Safety
/// Caller must ensure this is only called on x86_64 with SHA-NI support.
#[inline]
pub fn sha256d_midstate_asm(midstate: &Midstate, tail: &[u8; 16], nonce: u32) -> [u8; 32] {
    // Runtime CPU detection - if SHA-NI not available, fall back to generic
    if !is_x86_feature_detected!("sha") || !is_x86_feature_detected!("sse4.1") || !is_x86_feature_detected!("ssse3") {
        return super::generic::sha256d_midstate(midstate, tail, nonce);
    }

    // Build the last block (64 bytes):
    // [0..16]:   tail (16 bytes from header)
    // [16..20]:  nonce (4 bytes, little-endian)
    // [20]:      0x80 (SHA-256 padding: single 1-bit)
    // [21..62]:  0x00 (padding zeros)
    // [62..64]:  0x02 0x80 (message length in bits: 80*8 = 640 = 0x0280 big-endian)
    let mut block = [0u8; 64];
    block[..16].copy_from_slice(tail);
    block[16..20].copy_from_slice(&nonce.to_le_bytes());
    block[20] = 0x80;
    block[62] = 0x02;
    block[63] = 0x80;

    // First hash (continuing from midstate)
    let mut state = midstate.state;
    // SAFETY: We checked SHA-NI is available above
    unsafe { transform_asm(&mut state, &block) };
    let first_hash = state_to_bytes(&state);

    // Second hash (starting from H_INIT)
    // Block format for 256-bit (32-byte) message:
    // [0..32]:   first_hash (32 bytes)
    // [32]:      0x80 (padding)
    // [33..62]:  0x00 (padding zeros)
    // [62..64]:  0x01 0x00 (256 bits = 0x0100 big-endian)
    let mut state = H_INIT;
    let mut block2 = [0u8; 64];
    block2[..32].copy_from_slice(&first_hash);
    block2[32] = 0x80;
    block2[62] = 0x01;
    block2[63] = 0x00;
    // SAFETY: We checked SHA-NI is available above
    unsafe { transform_asm(&mut state, &block2) };

    state_to_bytes(&state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::sha256d_midstate_generic;

    #[test]
    fn test_asm_matches_generic() {
        // Only run if SHA-NI is available
        if !is_x86_feature_detected!("sha") || !is_x86_feature_detected!("sse4.1") || !is_x86_feature_detected!("ssse3") {
            println!("SHA-NI not available, skipping asm test");
            return;
        }

        // Test with various inputs
        let test_cases = vec![
            ([0x00u8; 64], [0x00u8; 16], 0u32),
            ([0xFFu8; 64], [0xFFu8; 16], 0u32),
            ([0xABu8; 64], [0x12u8; 16], 0xDEADBEEFu32),
            ([0x42u8; 64], [0x33u8; 16], 12345u32),
        ];

        for (i, (header, tail, nonce)) in test_cases.iter().enumerate() {
            let midstate = Midstate::from_header_prefix(header);
            let generic_hash = sha256d_midstate_generic(&midstate, tail, *nonce);
            let asm_hash = sha256d_midstate_asm(&midstate, tail, *nonce);

            assert_eq!(
                generic_hash, asm_hash,
                "Test case {}: asm hash mismatch.\nGeneric: {}\nAsm:     {}",
                i,
                hex::encode(generic_hash),
                hex::encode(asm_hash)
            );
        }
    }

    #[test]
    fn test_asm_transform_matches_generic() {
        if !is_x86_feature_detected!("sha") || !is_x86_feature_detected!("sse4.1") || !is_x86_feature_detected!("ssse3") {
            println!("SHA-NI not available, skipping transform test");
            return;
        }

        let test_blocks: Vec<[u8; 64]> = vec![
            [0x00; 64],
            [0xFF; 64],
            [0xAB; 64],
            std::array::from_fn(|i| i as u8),
        ];

        for (i, block) in test_blocks.iter().enumerate() {
            let mut state_generic = H_INIT;
            let mut state_asm = H_INIT;

            super::super::generic::transform(&mut state_generic, block);
            unsafe { transform_asm(&mut state_asm, block) };

            assert_eq!(
                state_generic, state_asm,
                "Test case {}: transform mismatch.\nGeneric: {:08x?}\nAsm:     {:08x?}",
                i, state_generic, state_asm
            );
        }
    }

    #[test]
    fn test_asm_different_nonces() {
        if !is_x86_feature_detected!("sha") || !is_x86_feature_detected!("sse4.1") || !is_x86_feature_detected!("ssse3") {
            println!("SHA-NI not available, skipping nonce test");
            return;
        }

        let header = [0xAB; 64];
        let midstate = Midstate::from_header_prefix(&header);
        let tail = [0x12; 16];

        let hash1 = sha256d_midstate_asm(&midstate, &tail, 0);
        let hash2 = sha256d_midstate_asm(&midstate, &tail, 1);
        let hash3 = sha256d_midstate_asm(&midstate, &tail, 0xFFFFFFFF);

        assert_ne!(hash1, hash2, "Different nonces should produce different hashes");
        assert_ne!(hash2, hash3, "Different nonces should produce different hashes");
        assert_ne!(hash1, hash3, "Different nonces should produce different hashes");
    }
}
