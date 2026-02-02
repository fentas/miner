# Research Test Log v19: Meet-in-the-Middle & Biclique Cryptanalysis

**Date:** 2026-02-02
**Focus:** MitM attacks, Biclique structures, Message schedule weakness, Neutral bits
**Goal:** Test advanced cryptanalytic techniques claimed to provide "shortcuts"
**Test file:** `tests/research_v19_test.rs`

---

## Background

Advanced cryptanalysis explores several "shortcut" approaches:

1. **Meet-in-the-Middle (MitM)** - Compute forward and backward, find collision in middle
2. **Biclique Cryptanalysis** - Lock rounds with mathematical structures
3. **Message Schedule Weakness** - Exploit linearity in W expansion
4. **Neutral Bits** - Find input bits with delayed avalanche
5. **SAT-CAS Hybrid** - Combine SAT solvers with algebraic analysis

---

## Test Results

### Test 1: Meet-in-the-Middle Backward Computation

Can we compute SHA-256 rounds backward?

**Single-Round Inversion:**
| Metric | Value |
|--------|-------|
| Perfect inversions | 10,000 / 10,000 (100%) |
| Average error | 0.00 bits |

**Multi-Round Backward (when W is known):**
| Rounds | Perfect Inversions | Avg Error |
|--------|-------------------|-----------|
| 1 | 1000/1000 | 0.0 bits |
| 2 | 1000/1000 | 0.0 bits |
| 4 | 1000/1000 | 0.0 bits |
| 8 | 1000/1000 | 0.0 bits |
| 16 | 1000/1000 | 0.0 bits |
| 32 | 1000/1000 | 0.0 bits |

**Key Finding:** Backward computation works perfectly - **IF you know W**.

**The Circular Dependency Problem:**
```
Forward: Message M → W[0..63] → Output hash
Backward: REQUIRES W[0..63] to invert each round

To go backward, you need W
To get W, you need M
To get M, you need to break the hash!
```

**Why MitM Fails:**
- The message schedule COUPLES all bits together
- You can't treat first and second halves independently
- 2^256 × table_lookup ≈ 2^256 (no speedup)

---

### Test 2: Biclique Structure Search

Searching for biclique-compatible state/message pairs:

**Single-Bit State Differences (4 rounds):**
| Metric | Value |
|--------|-------|
| Predictable (≤8 bits) | 0 / 256 |
| Output diff distribution | 9-128 bits |

**Compensating Message Differences:**
| Trials | Biclique-like | Rate |
|--------|---------------|------|
| 10,000 | 0 | 0.0000% |

**Output Difference Distribution:**
| Bits Changed | Cases |
|--------------|-------|
| 9 | 1 |
| 17-28 | 14 |
| 100+ | 241 |

**Biclique Attack Reality:**
1. **PSEUDO-preimage ≠ preimage** - Allows changing IV
2. **Covers only 4 rounds** - "52-round attack" means 52 rounds *analyzed*
3. **Complexity: 2^255.5** - Speedup of only 1.4× (meaningless)
4. **No attack on 64 rounds** - 12-round security margin

---

### Test 3: Message Schedule Weakness

Testing the linearity of W[16..63] expansion:

**Dependency Strength (1-bit flip in W[input]):**
| Input | W[16-23] | W[24-31] | W[32-39] | W[40-47] | W[48-55] | W[56-63] |
|-------|----------|----------|----------|----------|----------|----------|
| W[0] | 3.9 | 13.3 | 16.0 | 16.0 | 16.0 | 16.0 |
| W[7] | 0.8 | 13.3 | 16.0 | 16.0 | 16.0 | 16.0 |
| W[14] | 6.1 | 15.0 | 16.0 | 16.0 | 16.0 | 16.0 |

**XOR Approximation Error:**
| Word Range | Avg Error | % of 32 bits |
|------------|-----------|--------------|
| W[16-23] | 16.00 | 50.0% |
| W[24-31] | 16.00 | 50.0% |
| W[32-63] | 16.00 | 50.0% |

**Word-Pair Correlations:**
| Pair | Correlation |
|------|-------------|
| Best | 0.5092 |
| Worst | 0.4908 |
| Average | **0.5001** |
| Expected (random) | 0.5000 |

**Key Finding:** The message schedule IS relatively linear, but this doesn't help:
- XOR approximation has 50% error (random)
- The round function overwhelms any W weakness
- Correlations are essentially random (0.5001 ≈ 0.5000)

---

### Test 4: Neutral Bits Detection

Finding input bits with delayed avalanche:

**Most "Neutral" Bits (W[4] - enters at round 4):**
| Bit Position | After 4r | After 8r | After 12r | After 20r |
|--------------|----------|----------|-----------|-----------|
| W[4] bit 0 | 0.00 | 88.42 | 127.61 | 127.94 |
| W[4] bit 1-31 | 0.00 | ~88 | ~128 | ~128 |

**Neutral Bit Counts (threshold: <64 bit impact):**
| Rounds | Neutral Bits | Of 512 |
|--------|--------------|--------|
| 4 | 480 | 93.8% |
| 8 | 352 | 68.8% |
| 12 | 224 | 43.8% |
| 16 | 96 | 18.8% |
| **20** | **0** | **0%** |

**Key Finding:** By round 20, ALL input bits fully avalanche.

**Why Neutral Bits Don't Help Mining:**
- Mining changes NONCE, not arbitrary bits
- Nonce is at a FIXED position (bytes 76-79)
- Nonce enters at round ~2 and fully avalanches by round 4
- Neutral bits only help collision attacks (can choose all bits)

---

### Test 5: 52-Round Attack Reality Check

**The Published Attack (Khovratovich et al., 2011):**

| Aspect | Reality |
|--------|---------|
| Attack Type | **Pseudo-preimage** (not preimage) |
| Rounds | 52 / 64 |
| Complexity | 2^255.5 |
| Speedup | 1.41× |
| Practical? | **NO** |

**Complexity Comparison:**
```
Brute force:  2^256 = 3.7 × 10^69 years
Biclique:     2^255 = 1.8 × 10^69 years
Both:         ~10^59 × age of universe
```

**Full 64-Round SHA-256:**
- **No preimage attack exists**
- **No pseudo-preimage attack exists**
- **No collision faster than 2^128 birthday**
- Security margin: 12 rounds (pseudo), 33 rounds (collision)

**Why Mining is Immune:**
- Mining is partial preimage (k leading zeros)
- Difficulty ~2^80, not 2^256
- 1.4× speedup on 2^80 = 2^79.5 (still ~80 bits)
- ASIC gives 1,000,000× speedup (actually useful)

---

### Test 6: Combined Approach Complexity

**Proposed "Pro" Path Analysis:**

| Step | Claim | Reality |
|------|-------|---------|
| SAT for Biclique | Find 4-round structure | 2^3200 search space (impossible) |
| Linearization 20r | Approximate before/after | Random by round 4 (useless) |
| FPGA | Speed up computation | 10× on 2^256 = 2^252 (impossible) |

**Why the Combined Approach Fails:**

1. **Bicliques don't reduce complexity meaningfully**
   - Best case: 2^255.5 vs 2^256
   - For mining at 2^80: irrelevant

2. **Linearization doesn't work past 4 rounds**
   - "Fuzzy math" is 100% fuzzy (random)
   - Cannot combine "random" with "biclique"

3. **SAT solvers can't find bicliques**
   - Bicliques in papers found by mathematical analysis
   - SAT search space: 2^3200+

4. **FPGA doesn't change the math**
   - Faster operations × same count = still infeasible

---

## Analysis: The "Pro Path" Dead End

**What Actually Gives an Edge in Mining:**

| Approach | Speedup | Works? |
|----------|---------|--------|
| ASIC design | ~1,000,000× | ✓ |
| SHA-NI instructions | ~10× | ✓ |
| Parallelization | Linear with cores | ✓ |
| Cheap electricity | Cost reduction | ✓ |
| Biclique attack | 1.4× | ✗ |
| MitM attack | 0× (circular) | ✗ |
| Linearization | 0× (random) | ✗ |
| SAT/SMT solving | 0× (2^200+) | ✗ |

**The Billion-Dollar Verdict:**
Billions of dollars in mining R&D have explored these cryptanalytic ideas. The result: pure brute force with optimized hardware remains the only viable approach.

---

## Conclusion

**MitM & Biclique Cryptanalysis: NEGATIVE**

| Approach | Limitation |
|----------|------------|
| MitM | Circular dependency (needs W to invert) |
| Biclique | 1.4× speedup (meaningless at 2^256 or 2^80) |
| Message Schedule | 50% error (random), overwhelmed by rounds |
| Neutral Bits | Only helps collision, not mining |
| 52-Round Attack | Pseudo-preimage only, 12-round margin |
| Combined "Pro" Path | All components fail independently |

**SHA-256's Security Confirmed:**
- 64 rounds with full security margin
- No known attack faster than brute force
- The "cutting edge" is hardware, not cryptanalysis

---

## Raw Output

```
MitM backward: Perfect when W known, circular dependency otherwise
Biclique compensation: 0/10000 found
Message schedule: 0.5001 correlation (random)
Neutral bits: 0 remaining after 20 rounds
52-round attack: 2^255.5 complexity (meaningless)
Combined approach: All components fail
```
