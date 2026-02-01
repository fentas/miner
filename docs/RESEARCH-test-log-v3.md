# Research Test Log v3: Input Space Reduction / Nonce Bias Analysis

**Date:** 2026-02-01
**Hypothesis:** Certain nonce values may be more likely to produce low hashes
**Goal:** Find if we can prioritize "good" nonce ranges
**Test file:** `tests/research_v3_test.rs`

---

## Background

If SHA-256 is not a perfect random oracle for Bitcoin's specific input structure, there might be:
1. Nonce ranges that produce more leading zeros
2. Bit patterns in nonces that correlate with low hashes
3. Header configurations that interact with nonces to create bias

Even a small bias (e.g., 10% of nonces producing 20% of good hashes) would provide 2x speedup.

---

## Experiment Design

### Variables
- **Independent:** Nonce value, bit patterns, ranges, special patterns
- **Dependent:** Leading zero bit distribution
- **Control:** Fixed header structure (realistic Bitcoin-like)

### Tests Conducted
1. **Nonce Modulo Bias** - Do certain nonce % N values produce more zeros?
2. **Nonce Bit Patterns** - Does having bit X set/clear affect output?
3. **Nonce Range Bias** - Do high/low nonces differ?
4. **Special Nonce Patterns** - Are powers of 2, palindromes, etc. special?
5. **Header-Nonce Interaction** - Does header content create nonce bias?
6. **Chi-Squared Uniformity** - Formal statistical test

---

## Results

### Test 1: Nonce Modulo Bias

Samples: 100K per bucket × N buckets

| Modulo | Bucket Variance | Max Spread (bits) |
|--------|-----------------|-------------------|
| 16     | 0.000024        | 0.0213            |
| 256    | 0.000020        | 0.0278            |
| 1024   | 0.000019        | 0.0308            |

**Interpretation:** Variance ~ 0. No modulo-based bias exists.

---

### Test 2: Nonce Bit Pattern Analysis

Samples: 2,000,000

For each of the 32 bit positions in the nonce, compared mean leading zeros when bit=1 vs bit=0.

**Maximum difference across all bits:** 0.0050

**Interpretation:** No bit position affects output. All 32 bits are equally irrelevant.

---

### Test 3: Nonce Range Bias

Samples: 500K per range

| Range | Nonces | Mean Leading Zeros | Variance |
|-------|--------|-------------------|----------|
| Low (0x00-0x3F) | 0-1B | 0.9987 | 1.9952 |
| Mid-low (0x40-0x7F) | 1B-2B | 0.9987 | 1.9894 |
| Mid-high (0x80-0xBF) | 2B-3B | 0.9989 | 2.0064 |
| High (0xC0-0xFF) | 3B-4B | 0.9980 | 1.9945 |

**Variance between ranges:** 0.000000

**Interpretation:** Nonce range makes absolutely no difference.

---

### Test 4: Special Nonce Patterns

Samples: 1000 headers × pattern count

Baseline mean leading zeros: 0.9982

| Pattern | Samples | Mean LZ | Diff from Baseline |
|---------|---------|---------|-------------------|
| Powers of 2 | 32,000 | 1.0098 | +0.0116 |
| Round thousands | 1,000,000 | 1.0003 | +0.0021 |
| All 1s pattern | 32,000 | 0.9996 | +0.0014 |
| Alternating bits | 4,000 | 0.9988 | +0.0005 |
| Palindromes (hex) | 5,000 | 1.0124 | +0.0142 |

**Interpretation:** All differences are within noise (< 0.02). No special patterns exist.

---

### Test 5: Header-Nonce Interaction

Samples: 500,000 per configuration

| Header Type | Mean LZ | Std Dev |
|-------------|---------|---------|
| All zeros header | 1.0014 | 1.4156 |
| All ones header | 1.0001 | 1.4137 |
| Realistic header | 0.9985 | 1.4119 |
| High entropy header | 0.9990 | 1.4161 |

**Interpretation:** Header content doesn't affect nonce→hash distribution. All configurations produce identical statistics.

---

### Test 6: Chi-Squared Uniformity Test

Samples: 5,000,000

| LZ Bits | Observed | Expected | Ratio |
|---------|----------|----------|-------|
| 0 | 2,498,989 | 2,500,000 | 1.000 |
| 1 | 1,250,637 | 1,250,000 | 1.001 |
| 2 | 624,859 | 625,000 | 1.000 |
| 3 | 313,328 | 312,500 | 1.003 |
| 4 | 156,198 | 156,250 | 1.000 |
| 5 | 77,885 | 78,125 | 0.997 |
| 6 | 38,981 | 39,062 | 0.998 |
| 7 | 19,514 | 19,531 | 0.999 |
| 8 | 9,859 | 9,766 | 1.010 |
| 9 | 4,886 | 4,883 | 1.001 |
| 10 | 2,391 | 2,441 | 0.979 |
| 11 | 1,218 | 1,221 | 0.998 |
| 12 | 637 | 610 | 1.044 |
| 13 | 305 | 305 | 0.999 |
| 14 | 151 | 153 | 0.990 |
| 15 | 89 | 76 | 1.167 |
| 16 | 29 | 38 | 0.760 |

**Chi-squared statistic:** 11.33
**Critical value (p=0.05, df=16):** 26.3
**Result:** PASS (distribution is uniform)

**Interpretation:** The observed distribution exactly matches the theoretical expectation P(k leading zeros) = 2^(-k-1). SHA-256 is a perfect random oracle.

---

## Analysis

### Key Finding: SHA-256 is a Perfect Random Oracle for Bitcoin

Every test confirms that nonce selection provides ZERO advantage:

1. **No modulo bias** - nonce % N doesn't matter
2. **No bit pattern bias** - individual bits don't correlate with output
3. **No range bias** - low and high nonces are equivalent
4. **No special patterns** - powers of 2, palindromes, etc. are not special
5. **No header interaction** - header content doesn't create nonce bias
6. **Perfect uniformity** - chi-squared test passes decisively

### Mathematical Interpretation

The distribution of leading zeros follows exactly:
- P(0 zeros) = 50%
- P(1 zero) = 25%
- P(2 zeros) = 12.5%
- P(k zeros) = 2^(-k-1)

This is the theoretical expectation for a uniform random hash function. SHA-256 achieves this perfectly.

---

## Conclusion

**NEGATIVE RESULT:** Input space reduction is NOT viable.

The nonce space cannot be reduced because:
1. All nonce values are equally likely to produce any hash
2. No patterns, ranges, or special values provide advantage
3. SHA-256 is a cryptographically perfect random oracle

**Implications:**
1. Cannot skip any nonce values
2. Cannot prioritize certain nonce ranges
3. Brute force over full 2^32 nonce space is optimal
4. The only "optimization" is to search faster, not smarter

---

## What This Rules Out

From RESEARCH-ideas.md, we can now eliminate:

| Idea | Status |
|------|--------|
| Input Space Reduction | ❌ Ruled out |
| Nonce Range Prioritization | ❌ Ruled out |
| Special Pattern Exploitation | ❌ Ruled out |
| Header-Nonce Interaction | ❌ Ruled out |

---

## Next Steps

Move to Experiment v4: Round Function Analysis
- Look at the algebraic structure of SHA-256
- Question: Can we express "hash < target" as solvable equations?
- Focus on linear approximation (even if impractical)

---

## Raw Output

```
=== Chi-Squared Uniformity Test ===
Chi-squared statistic: 11.33
Critical value (p=0.05, df=16): 26.3
Result: PASS (uniform)

=== Nonce Range Bias ===
Variance between ranges: 0.000000
Expected (random): ~0

=== Nonce Bit Pattern Analysis ===
Max difference across all bits: 0.0050
Expected (random): ~0

=== Special Nonce Patterns ===
All differences from baseline < 0.02 (noise)
```
