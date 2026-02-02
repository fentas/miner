# Research Test Log v24: Wild & Loco Theories

**Date:** 2026-02-02
**Focus:** Unconventional, creative hypotheses - "thinking outside the box"
**Goal:** Test wild theories that probably won't work but are worth exploring
**Test file:** `tests/research_v24_test.rs`

---

## Background

After 23 experiments proving SHA-256 is a perfect random oracle, we go CRAZY with unconventional theories. These are "what if?" ideas that cryptographers would dismiss but are worth testing empirically for completeness.

**Philosophy:** Sometimes breakthroughs come from testing "obviously wrong" ideas. If we don't test them, we don't know for sure.

---

## Wild Theories Tested

### Test 1: Hamming Weight Destiny

**Theory:** Do valid nonces cluster at certain Hamming weights (number of 1-bits)?

**Hypothesis:** If nonces with specific bit counts (e.g., 16 ones) are "luckier", we could prioritize them.

**Results (10M trials, 16-zero target):**

| Hamming Weight | All Nonces | Valid | Ratio vs Expected |
|---------------:|-----------:|------:|------------------:|
| 6 | 120,859 | 3 | 1.59x |
| 10 | 1,395,380 | 22 | 1.01x |
| 13 | 1,348,929 | 24 | 1.14x |
| 16 | 272,395 | 6 | 1.41x |
| 19 | 9,216 | 1 | 6.96x |

**Statistical Analysis:**
```
Average HW of valid nonces: 11.506
Expected (uniform random): 16.0
Difference: -4.494 (-28.08%)

Chi-squared: 16.18 (reject if > 46.0)
```

**Verdict:** NOT SIGNIFICANT

The -28% HW difference is a sampling artifact (nonces 0-10M have lower average HW than full 32-bit space). Chi-squared confirms uniform distribution.

---

### Test 2: Gray Code Walk

**Theory:** Does single-bit increment (Gray code) produce different hash distributions?

**Hypothesis:** If adjacent Gray code values have more similar intermediate SHA-256 states, maybe consecutive hashes could be "correlated".

**Results (5M trials each, 12-zero target):**

| Method | Valid | Avg Zeros |
|--------|------:|----------:|
| Binary | 1,235 | 1.0007 |
| Gray | 1,223 | 1.0000 |

Gray/Binary ratio: **0.99x**

**Consecutive Hash Similarity:**
```
Avg same bytes between consecutive Gray hashes: 0.125
Expected (random): 1.0
```

**Verdict:** NO DIFFERENCE

Gray code provides no advantage. Consecutive hashes are just as random as binary increment.

---

### Test 3: Message Schedule Fingerprint

**Theory:** Can patterns in W[16:63] (expanded message schedule) predict validity?

**Hypothesis:** The message expansion might create distinguishable patterns for "good" nonces.

**Results (5M trials, 16-zero target):**

| Category | Avg Entropy (W popcount sum) |
|----------|-----------------------------:|
| All | 772.88 |
| Valid | 771.96 |

Difference: 0.91 (0.12%)

**W[16] Distribution:**
- Unique W[16] values in all: 1
- W[16] values with >2x valid rate: 0

**Verdict:** NOT SIGNIFICANT

Message schedule entropy is identical for valid and invalid hashes. No W[16] value is "lucky".

---

### Test 4: Glitch Variance

**Theory:** Do some nonces create fewer carry propagation "glitches"?

**Hypothesis:** Nonces that cause fewer carry cascades might have different hash properties.

**Results (2M trials, 14-zero target):**

| Category | Avg Glitches | Std Dev |
|----------|-------------:|--------:|
| All | 34.38 | 2.24 |
| Valid | 34.08 | 2.26 |

Difference: 0.31 (0.89%)

**Low-glitch Analysis:**
```
Low-glitch nonces (<32): 198,246 total, 15 valid
Expected valid: 12.7
Ratio: 1.18x (not significant)
```

**Verdict:** NOT SIGNIFICANT

Carry glitch count doesn't correlate with hash validity.

---

### Test 5: Prime Nonce Magic

**Theory:** Are prime number nonces statistically different?

**Hypothesis:** SHA-256's K constants come from primes. Maybe there's "resonance" with prime nonces.

**(This is testing pure superstition)**

**Results (10M trials, 12-zero target):**

| Category | Count | Valid | Rate | Avg Zeros |
|----------|------:|------:|-----:|----------:|
| Prime | 664,579 | 156 | 0.0235% | 0.9999 |
| Non-Prime | 9,335,419 | 2,303 | 0.0247% | 1.0005 |

Prime/Non-Prime ratio: **0.95x** (primes are WORSE)

**Verdict:** CONFIRMED NO MAGIC

Primes are not special. SHA-256 doesn't care about number-theoretic properties.

---

### Test 6: Bit Position Dominance

**Theory:** Are all 32 nonce bits equally influential?

**Hypothesis:** Maybe high bits (e.g., bit 31) have more effect on final zeros than low bits (bit 0).

**Results (100K base nonces, single-bit flip test):**

| Bit | Mean Change | Std Dev |
|----:|------------:|--------:|
| 0 | +0.0000 | 2.018 |
| 15 | -0.0086 | 2.008 |
| 25 | +0.0029 | 2.013 |
| 31 | +0.0012 | 2.011 |

**Summary:**
```
Average influence: -0.0012
Min influence: -0.0086 (bit 15)
Max influence: +0.0029 (bit 25)
Spread: 0.0115
```

**Verdict:** ALL BITS EQUAL

No dominant bit position. The spread (0.0115) is negligible noise.

---

### Test 7: Reverse Birthday Attack

**Theory:** Do valid hashes cluster in output space (non-zero bytes)?

**Hypothesis:** If valid hashes share similar "fingerprints" in bytes 4-31, we could use this for filtering.

**Results (10M trials, 16-zero target):**

```
Valid hashes found: 156
Unique fingerprints: 156
Collisions (>1 hit): 0
Expected (birthday): 0.0
```

**Verdict:** NO CLUSTERING

Each valid hash has a completely unique fingerprint. No "attractor" regions in output space.

---

### Test 8: Compression Function Orbits

**Theory:** Do repeated hash iterations show cyclic patterns?

**Hypothesis:** If hash(hash(hash(...))) eventually cycles, we could exploit this.

**Results (1000 samples, 1000 iterations each):**

```
Cycles found: 0 / 1000
First-step divergence rate: 0.5003 (expected: 0.50)
```

**Verdict:** NO CYCLES

SHA-256's orbit is effectively infinite. Perfect 50% avalanche on each iteration.

---

### Test 9: Nonce XOR Relationships

**Theory:** Do valid nonces have XOR patterns?

**Hypothesis:** If valid nonces XOR to special values (e.g., powers of 2), we could use this for search.

**Results (328 valid nonces, 53K pairs analyzed):**

**XOR Popcount Distribution:**
- Follows expected binomial(32, 0.5) distribution
- Most common XOR values appear only 2 times each
- Expected: uniform (1 appearance per value)

**Verdict:** NO PATTERN

XOR values are uniformly distributed. Valid nonces have no relationship.

---

### Test 10: Timestamp Wiggle

**Theory:** Are some timestamp values "luckier" than others?

**Hypothesis:** Bitcoin allows ±2 hour timestamp flexibility. Maybe some seconds are better.

**Results (500K nonces × 8 timestamps, 14-zero target):**

| Timestamp | Valid | Deviation |
|-----------|------:|----------:|
| 0x5F5E0000 | 26 | -12.6% |
| 0x5F5E0001 | 44 | +47.9% |
| 0x5F5E0010 | 16 | -46.2% |
| 0x5F5E0100 | 28 | -5.9% |
| 0x60000000 | 34 | +14.3% |

Spread: 94.1% (min 16 to max 44)

**Statistical Reality Check:**
```
Expected valid per timestamp: ~30
Standard deviation (Poisson): √30 ≈ 5.5
95% confidence interval: 30 ± 11 = [19, 41]

Range 16-44 is within ~2.5σ for 8 samples
```

**Verdict:** NOT SIGNIFICANT (Poisson noise)

The apparent "94% spread" is just statistical variance. With ~30 expected events per timestamp, variation from 16 to 44 is completely normal.

---

## Summary Table

| Wild Theory | Result | Key Finding |
|-------------|--------|-------------|
| Hamming Weight Destiny | ❌ NEGATIVE | Chi-sq = 16.18 (need >46 for significance) |
| Gray Code Walk | ❌ NEGATIVE | 0.99x ratio = identical |
| Message Schedule Fingerprint | ❌ NEGATIVE | 0.12% entropy difference = noise |
| Glitch Variance | ❌ NEGATIVE | 0.89% difference = noise |
| Prime Nonce Magic | ❌ NEGATIVE | Primes are 0.95x WORSE |
| Bit Position Dominance | ❌ NEGATIVE | 0.0115 spread = all bits equal |
| Reverse Birthday | ❌ NEGATIVE | 0 collisions = no clustering |
| Compression Orbits | ❌ NEGATIVE | 0 cycles found |
| XOR Relationships | ❌ NEGATIVE | Uniform distribution |
| Timestamp Wiggle | ❌ NEGATIVE | Poisson noise (94% spread = 2.5σ) |

---

## Conclusion

**Wild & Loco Theories: ALL NEGATIVE**

Even the most creative, unconventional theories fail to find any exploitable pattern in SHA-256. The hash function is:

1. **Indifferent to input structure** - Primes, Hamming weights, bit positions don't matter
2. **Perfectly avalanching** - Gray code is no better than binary
3. **Memory-free** - No orbital cycles or clustering
4. **Uniformly random** - Message schedule doesn't leak information
5. **Timestamp-agnostic** - No "lucky" seconds

**The Wild Theory Test confirms:** SHA-256 is cryptographically perfect. There are no hidden patterns, no lucky numbers, no special structures. Every nonce is equally (un)likely to produce a valid hash.

**If any of these theories had worked, it would have been a major cryptographic discovery.**

---

## Raw Output

```
Hamming Weight: Chi-sq 16.18 (NS), avg HW diff -4.5 (sampling artifact)
Gray Code: 0.99x ratio, 0.125 byte similarity (random)
Message Schedule: 0.12% entropy diff (NS)
Glitch Variance: 0.89% diff, low-glitch ratio 1.18x (NS)
Prime Nonces: 0.95x ratio (primes worse, as expected)
Bit Position: 0.0115 spread (all equal)
Birthday Clustering: 0 collisions (no clustering)
Compression Orbits: 0 cycles, 50.03% divergence (perfect)
XOR Relationships: binomial distribution (uniform)
Timestamp Wiggle: 94% spread = Poisson noise
```
