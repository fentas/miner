# Research Test Log v20: Neutral Bits & MitM Practical Analysis

**Date:** 2026-02-02
**Focus:** True neutral bits, MitM table requirements, nonce position, practical speedup
**Goal:** Verify claims about "25% of the puzzle free" and MitM complexity reduction
**Test file:** `tests/research_v20_test.rs`

---

## Background

Advanced cryptanalysis claims:

1. **Local Reversibility** - Single rounds are invertible when W is known
2. **MitM Complexity** - Reduces 2^256 to 2^128 via table lookup
3. **Neutral Bits** - Some input bits don't affect early rounds (25% savings claimed)
4. **Pre-computation** - Calculate first 15 rounds once, vary neutral bits

---

## Test Results

### Test 1: True Neutral Bits (Zero State Difference)

Testing which bits have EXACTLY zero impact on state through N rounds:

**Neutral Bit Count Summary:**
| Round | Bits with Zero-Diff | Percentage |
|-------|---------------------|------------|
| 0 | 512 | 100.0% |
| 1 | 480 | 93.8% |
| 2 | 448 | 87.5% |
| 4 | 384 | 75.0% |
| 8 | 256 | 50.0% |
| 12 | 128 | 25.0% |
| 15 | 32 | 6.2% |
| **16** | **0** | **0.0%** |

**Key Finding:**
- W[i] is neutral through round i-1 (confirmed)
- W[15] is neutral through round 14 (all 32 bits)
- By round 16, NO bits are neutral

---

### Test 2: Neutral Bit Speedup Calculation

**Theoretical Savings (if you could choose which bits to vary):**
| Neutral Through | Pre-compute | Vary | Savings |
|-----------------|-------------|------|---------|
| Round 4 | 6.2% | 93.8% | 6.25% |
| Round 8 | 12.5% | 87.5% | 12.50% |
| Round 12 | 18.8% | 81.2% | 18.75% |
| Round 15 | 23.4% | 76.6% | **23.44%** |

**Message Schedule Coupling:**
```
W[16] = σ₁(W[14]) + W[9] + σ₀(W[1]) + W[0]   <- W[15] NOT used
W[17] = σ₁(W[15]) + W[10] + σ₀(W[2]) + W[1]  <- W[15] USED via σ₁
```

Actual savings from W[15] neutrality: **23.4%** (not 25%)

**CRITICAL FOR MINING:**
- Nonce is at bytes 76-79 of block header
- In Block 2's SHA-256, nonce = **W[3]**
- W[3] enters at round 3 → neutral through rounds 0-2 only

**ACTUAL MINING SAVINGS: 4.7%** (not 25%)

---

### Test 3: MitM Table Size Requirements

**Memory Requirements for MitM Tables:**
| Table Size | Storage | Practical? |
|------------|---------|------------|
| 2^16 | 2.1 MB | Yes |
| 2^24 | 537 MB | Yes |
| 2^32 | 137 GB | Difficult |
| 2^40 | 32 TB | No |
| 2^64 | 2^69 bytes | Impossible |
| 2^128 | 2^133 bytes | Impossible |

**For SHA-256 MitM with expected 1 match:**
- Need k + m = 256 (forward + backward)
- Optimal: 2^128 table, 2^128 backward
- Memory: 2^133 bytes (impossible)

**For mining at difficulty 2^80:**
- MitM table: 2^40 entries = 32 TB
- BUT: Backward computation requires the nonce (circular dependency)
- MitM CANNOT help mining

---

### Test 4: Nonce Position Impact (Mining Reality)

**Bitcoin Block Header Layout:**
| Bytes | Field | SHA-256 Block |
|-------|-------|---------------|
| 0-63 | Version, Prev Hash, Merkle (partial) | Block 1 |
| 64-79 | Merkle (end), Timestamp, Bits, **NONCE** | Block 2 |

**Nonce Position in Block 2:**
```
W[0] = Merkle root (last 4 bytes)
W[1] = Timestamp
W[2] = Bits (target)
W[3] = **NONCE**
W[4] = Padding (0x80...)
W[5-13] = Zeros
W[14-15] = Length
```

**Empirical Nonce Impact Test:**
| Round | Avg Diff (bits) | Status |
|-------|-----------------|--------|
| 0 | 0.00 | NEUTRAL ✓ |
| 1 | 0.00 | NEUTRAL ✓ |
| 2 | 0.00 | NEUTRAL ✓ |
| 3 | 0.00 | NEUTRAL ✓ |
| 4 | 4.02 | Affected |
| 5 | 25.72 | Affected |
| 6 | 56.78 | Affected |
| 7 | 88.96 | Fully avalanched |

**Key Finding:** Nonce is neutral through round 3, avalanche begins at round 4.

**BUT:** This is already exploited via **midstate optimization**:
- First SHA-256 block (bytes 0-63) is constant
- Mining software pre-computes this
- The "neutral bit" savings are ALREADY CAPTURED

---

### Test 5: Forward/Backward Balance Optimization

**MitM Split Optimization (Theoretical):**
| Split | Forward | Backward | Memory | Work |
|-------|---------|----------|--------|------|
| 24 | 24r | 40r | 2^160 | 2^160 |
| 32 | 32r | 32r | 2^128 | 2^128 |
| 40 | 40r | 24r | 2^96 | 2^96 |

Optimal split: Round 32 with 2^128 table (unfeasible)

**Why This Doesn't Help:**
1. Memory barrier: 2^128 states = impossible
2. Circular dependency: Backward needs W, which needs nonce
3. Mining difficulty is 2^80, not 2^256

---

## Analysis: Why Claims Don't Apply to Mining

### Claim: "Local reversibility allows backward computation"
- **Verified:** Single rounds invert perfectly when W is known
- **Problem:** W requires the message (nonce), which is unknown
- **Status:** TRUE but USELESS for attacks

### Claim: "MitM reduces 2^256 to 2^128"
- **Theory:** Store 2^128 forward, try 2^128 backward
- **Memory:** 2^133 bytes (impossible)
- **Mining:** Backward needs nonce (circular)
- **Status:** THEORETICALLY TRUE, PRACTICALLY IMPOSSIBLE

### Claim: "Neutral bits make 25% free"
- **Theory:** W[15] neutral through round 14 = 23% savings
- **Mining:** Nonce is W[3], neutral through round 2 = 4.7%
- **Reality:** Midstate optimization already captures this
- **Status:** ALREADY IMPLEMENTED

### Claim: "Pre-calculate first 15 rounds"
- **Applies to:** Collision attacks (choose all message bits)
- **Mining:** Cannot choose bits - nonce position fixed
- **Status:** NOT APPLICABLE

---

## Conclusion

**Neutral Bits & MitM Practical Analysis: NEGATIVE (for mining)**

| Claim | Theory | Mining Reality |
|-------|--------|----------------|
| Local reversibility | 100% accurate | Useless (needs W) |
| MitM complexity | 2^128 | Circular dependency |
| Neutral bits | 23% savings | 4.7% (already optimized) |
| Pre-computation | 15 rounds | 3 rounds (midstate) |

**The "cutting edge" claims apply to:**
- Collision attacks (can choose all message bits)
- Theoretical scenarios

**They do NOT apply to:**
- Preimage attacks (fixed message structure)
- Mining (nonce at fixed position)

**Mining optimization is HARDWARE optimization:**
- More parallel cores
- Lower power consumption
- Better clock speeds
- Midstate pre-computation (standard)

**There are NO algorithmic shortcuts for mining.**

---

## Raw Output

```
Neutral bits through round 15: 32/512 (W[15] only)
Neutral bits through round 16: 0/512
Nonce position: W[3] in Block 2
Nonce neutral window: rounds 0-3 (4 rounds)
Mining savings vs claim: 4.7% vs 25%
Midstate optimization: Already captures this
MitM for mining: Circular dependency (impossible)
```
