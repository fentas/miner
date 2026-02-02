# Research Test Log v22: Sweet Spot Bits & Round 60 Early Abort

**Date:** 2026-02-02
**Focus:** W[3]/W[4] "sweet spots", round 60 early abort, double SHA-256 impact
**Goal:** Test claims about "5-10% efficiency boost" and "skip rounds 1-15"
**Test file:** `tests/research_v22_test.rs`

---

## Background

Advanced mining claims suggest:

1. **W[3]/W[4] "Sweet Spots"** - Slowest avalanche, can skip rounds 1-15
2. **Round 60 Early Abort** - Check A register, abort if no zeros → 5-10% savings
3. **Nested Loop Optimization** - Outer loop full 64r, inner loop skips pre-computed rounds
4. **MitM for Mining** - Meet forward/backward waves in middle

**Critical Issue to Test:** Bitcoin uses **DOUBLE SHA-256**. Early abort claims ignore this.

---

## Test Results

### Test 1: W[3]/W[4] Sweet Spot Analysis

Testing which W[i] positions have "slow" avalanche:

**Avalanche Timing by W[i] Position:**
| W[i] | Round Enters | Round 4 Diff | Round 8 Diff | Round 16 Diff | Full Diff |
|------|--------------|--------------|--------------|---------------|-----------|
| W[0] | Round 0 | 92 bits | 120 bits | 130 bits | 125 bits |
| W[1] | Round 1 | 65 bits | 134 bits | 141 bits | 122 bits |
| W[2] | Round 2 | 30 bits | 129 bits | 128 bits | 130 bits |
| W[3] | Round 3 | 2 bits | 106 bits | 132 bits | 126 bits |
| **W[4]** | **Round 4** | **0 bits** ✓ | 108 bits | 129 bits | 129 bits |
| W[5] | Round 5 | 0 bits ✓ | 61 bits | 140 bits | 130 bits |
| ... | ... | ... | ... | ... | ... |
| W[15] | Round 15 | 0 bits ✓ | 0 bits | 3 bits | 140 bits |

**Key Finding:**
- W[i] is neutral through round i-1 (confirmed)
- W[3] enters at round 3 → neutral through rounds 0-2 only
- W[15] is most "neutral" but not relevant for mining

**For Bitcoin Mining:**
- Nonce is at W[3] in Block 2
- Only 3 rounds of neutrality (not 15)
- Already captured by midstate optimization

---

### Test 2: Round 60 Early Abort Analysis

**CRITICAL: Bitcoin uses SHA256(SHA256(header))**

Testing correlation between round 60 state and final hash:

**Part A - Single SHA-256 (NOT how Bitcoin works):**
| Metric | Value |
|--------|-------|
| Correlation (r60 A zeros vs final zeros) | **0.000412** |
| Interpretation | NONE |

**Part B - Double SHA-256 (ACTUAL Bitcoin):**
| Metric | Value |
|--------|-------|
| Correlation (first hash r60 vs final) | **0.004579** |
| Interpretation | NONE (as expected) |

**Part C - Second Hash Round 60:**
| Metric | Value |
|--------|-------|
| Correlation (second hash r60 vs final) | **-0.001281** |
| Interpretation | NONE |

**Cost Analysis (if round 60 abort worked):**
```
Work before abort decision: 64 (first hash) + 60 (second hash) = 124 rounds
Work saved by abort: 4 rounds (rounds 61-64 of second hash)
Maximum theoretical savings: 4/128 = 3.1%
Realistic savings: <1% (most nonces complete anyway)
```

---

### Test 3: Nested Loop Efficiency

**Claim:** "Skip re-calculation of Rounds 1-15 for massive percentage"

**Bitcoin Block Structure:**
```
┌──────────────────────────────────────┐
│ Block 1 (64 bytes) - CONSTANT        │
│ W[0-15]: version, prev_hash, merkle  │
│ → Computed ONCE as midstate          │
└──────────────────────────────────────┘
┌──────────────────────────────────────┐
│ Block 2 (16 bytes + padding)         │
│ W[0]: merkle_end (constant)          │
│ W[1]: timestamp  (constant per sec)  │
│ W[2]: bits       (constant)          │
│ W[3]: **NONCE**  (variable)          │
│ W[4-15]: padding (constant)          │
└──────────────────────────────────────┘
```

**Nonce Impact by Round:**
| Round | Nonces with Zero Diff | Can Skip? |
|-------|----------------------|-----------|
| 1 | 100.0% | YES |
| 2 | 100.0% | YES |
| 3 | 100.0% | **NO** (nonce enters) |
| 4 | 0.0% | NO |
| 5+ | 0.0% | NO |

**Reality:**
- Nonce is W[3], affects state starting at round 3
- Rounds 0-2: Can pre-compute (3 rounds, not 15)
- Marginal savings: 2/126 = **1.6%** (beyond midstate)

---

### Test 4: Double SHA-256 Impact

**The Fatal Flaw in Early Abort Claims:**

```
Bitcoin Hash: SHA256(SHA256(80-byte header))

Timeline of work:
┌─────────────────────────────────────────────────────────┐
│ First SHA-256                                          │
│ Block 1: Rounds 0-63 (constant → midstate)             │
│ Block 2: Rounds 0-63 (nonce varies → intermediate)     │
├─────────────────────────────────────────────────────────┤
│ Second SHA-256                                         │
│ Block 1: Rounds 0-63 (intermediate → final hash)       │
└─────────────────────────────────────────────────────────┘
```

**First Hash Zeros for Valid Final Hashes (8-zero target):**
| First Hash Zeros | Count | % of Valid |
|------------------|-------|------------|
| 0 | ~1040 | 5.3% |
| 1 | ~3800 | 19.4% |
| 2 | ~7500 | 38.3% |
| 3+ | ~7200 | 37.0% |

**Key Finding:** First hash zeros are uniformly distributed among valid hashes.
The first hash output is completely uncorrelated with final hash validity.

---

### Test 5: The 5-10% Efficiency Claim

**Direct Simulation (10M trials, 16-zero target):**

| Metric | Value |
|--------|-------|
| Valid hashes found | 136 |
| Abort threshold | A zeros ≤ 0 |
| Aborted | 49.97% |
| Rounds saved | 1.999 per attempt |
| Efficiency gain | **1.562%** |
| **Valid hashes missed** | **63 (46.3%)** |

**Verdict:** REJECTED
- Early abort MISSES nearly half of valid hashes
- Any abort strategy that misses valid hashes = lost revenue
- Maximum theoretical savings: 3.1%, not 5-10%

---

### Test 6: Meet-in-the-Middle Reality

**Collision vs Preimage:**
```
┌─────────────────────────────────────────────────────────────────┐
│ COLLISION: Find M1, M2 such that H(M1) = H(M2)                 │
│   - You CHOOSE both messages                                   │
│   - Birthday paradox: ~2^128 operations                        │
│   - MitM: forward from M1s, backward from M2s, meet at middle  │
│   - DOES NOT HELP MINING                                       │
├─────────────────────────────────────────────────────────────────┤
│ PREIMAGE: Given target T, find M such that H(M) < T            │
│   - Message structure is FIXED (Bitcoin header)                │
│   - Only nonce varies (32 bits)                                │
│   - Backward requires knowing message schedule W               │
│   - Circular dependency: need W to go backward, need M for W   │
│   - THIS IS MINING - MitM does NOT help                        │
└─────────────────────────────────────────────────────────────────┘
```

**Why MitM Can't Help Mining:**
1. Backward computation requires W (message schedule)
2. W is derived from M (message)
3. M contains the nonce (what we're searching for)
4. Circular dependency: can't go backward without knowing answer

**Double SHA-256 compounds the problem:**
- First hash output = random 256 bits
- These become input to second hash
- No way to "meet in middle" across two hash calls

---

## Analysis: Why Claims Fail

### The Double SHA-256 Problem

**ALL "early abort" claims ignore that Bitcoin uses double SHA-256:**

1. **First hash early abort (round 60):**
   - Cannot predict second hash output (complete avalanche)
   - Correlation: 0.004579 ≈ 0

2. **Second hash early abort (round 60):**
   - Only 4 rounds to save
   - Maximum savings: 3.1%
   - But 46.3% of valid hashes would be missed

### The W[3]/W[4] "Sweet Spot" Myth

| Claimed | Reality |
|---------|---------|
| "Skip rounds 1-15" | Skip rounds 0-2 only |
| "Massive percentage" | 2/126 = 1.6% |
| "Sweet spots" | Every W[i] enters at round i |

### The 5-10% Efficiency Myth

| Claimed | Reality |
|---------|---------|
| 5-10% savings | 1.56% max (and loses 46% of valid hashes) |
| "Safe" early abort | ALL abort strategies miss valid hashes |
| Predictive signal | Correlation ≈ 0 at all checkpoints |

---

## Conclusion

**Sweet Spot & Round 60 Abort: MIXED (mostly NEGATIVE)**

| Claim | Status |
|-------|--------|
| W[3]/W[4] are "sweet spots" | PARTIALLY TRUE (but irrelevant for mining) |
| Skip rounds 1-15 | FALSE (only 0-2 for nonce) |
| Round 60 early abort gives 5-10% | FALSE (max 3.1%, loses 46% of valid) |
| Nested loop optimization | ALREADY DEPLOYED (midstate) |
| MitM helps mining | FALSE (helps collisions only) |

**Critical Insight:** Bitcoin uses DOUBLE SHA-256
- Early abort in first hash: POINTLESS (no predictive power)
- Early abort in second hash: DANGEROUS (misses valid hashes)

**Key Findings:**

1. **Double SHA-256 breaks ALL early abort strategies** - first hash output is uncorrelated with final validity
2. **W[3] neutrality is only 3 rounds** - already captured by midstate
3. **ALL abort thresholds miss valid hashes** - 46.3% miss rate at best strategy
4. **Maximum theoretical savings: 3.1%** - not worth the complexity
5. **MitM requires knowing the answer** - circular dependency

---

## Raw Output

```
W[3]/W[4] neutrality: 3-4 rounds (not 15)
Round 60 correlation (single SHA-256): 0.000412 (none)
Round 60 correlation (double SHA-256): 0.004579 (none)
Early abort miss rate: 46.3% of valid hashes
Maximum savings: 3.1% (not 5-10%)
Nested loop marginal savings: 1.6%
MitM for mining: Circular dependency (impossible)
```
