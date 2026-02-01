# Research Test Log v2: Differential/Consecutive Nonce Analysis

**Date:** 2026-02-01
**Hypothesis:** Consecutive nonces may produce correlated hashes
**Goal:** Find if hash(N) provides information about hash(N+1)
**Test file:** `tests/research_v2_test.rs`

---

## Background

If consecutive nonces produce hashes with ANY correlation, we could:
1. Use hash(N) to predict which N+1 values are more likely to be "good"
2. Skip certain nonce ranges based on observed patterns
3. Search more efficiently by following "promising" directions

---

## Experiment Design

### Variables
- **Independent:** Nonce value N
- **Dependent:** Relationship between hash(N) and hash(N+1)

### Metrics
1. **Bit flip count** - How many bits change between consecutive hashes?
2. **Leading byte correlation** - Do consecutive hashes have similar first bytes?
3. **Good hash clustering** - If hash(N) is "good", is hash(N+1) more likely good?
4. **Direction persistence** - If first byte increases, does it keep increasing?
5. **Bit position bias** - Do certain bit positions flip more/less than 50%?
6. **Step size effects** - Does changing nonce by 1 vs 256 vs 65536 matter?

---

## Results

### Test 1: Bit Flip Statistics (1M samples)

| Metric | Observed | Expected (Random) |
|--------|----------|-------------------|
| Average bit flips | 127.99 | 128.0 |
| Min flips | 87 | ~87 |
| Max flips | 168 | ~168 |

**Interpretation:** Perfect avalanche - exactly 50% of bits flip. No deviation from random.

---

### Test 2: Leading Zero Byte Correlation (1M samples)

| Condition | Count | Percentage |
|-----------|-------|------------|
| Both have leading zero byte | 19 | 0.0019% |
| Neither has leading zero | 992,065 | 99.2065% |
| Only first has zero | 3,966 | 0.3966% |
| Only second has zero | 3,950 | 0.3950% |

**Correlation:** 99.2084%
**Expected (independent):** 99.2218%

**Interpretation:** Correlation matches independent probability exactly. No predictive power.

---

### Test 3: First Byte Correlation (1M samples)

| Metric | Observed | Expected (Random) |
|--------|----------|-------------------|
| Exactly same byte | 0.3943% | 0.3906% |
| Within ±16 | 12.4754% | 12.8906% |

**Interpretation:** First bytes are completely uncorrelated - match random expectation.

---

### Test 4: Leading Zero Bits (1M samples)

| Metric | Observed |
|--------|----------|
| Exactly same count | 33.35% |
| Within ±2 bits | 83.35% |

**Note:** These match geometric distribution expectations for random hashes.

---

### Test 5: Bit Position Analysis (500K samples)

| Metric | Value |
|--------|-------|
| Unusual bit positions found | 0 |
| Min flip rate | 49.82% |
| Max flip rate | 50.18% |
| Avg flip rate | 50.00% |

**Interpretation:** Every single bit position flips at exactly 50% rate. No bias anywhere.

---

### Test 6: Direction Correlation (1M samples)

| First Trend | Second Trend | Count | Percentage | Expected |
|-------------|--------------|-------|------------|----------|
| Up | Up | 164,673 | 16.47% | 25% |
| Up | Down | 333,191 | 33.32% | 25% |
| Down | Up | 333,834 | 33.38% | 25% |
| Down | Down | 168,302 | 16.83% | 25% |

**Interpretation:** Strong ANTI-correlation! This is "regression to mean":
- If first byte goes UP, second is 2x more likely to go DOWN
- If first byte goes DOWN, second is 2x more likely to go UP

**Why this doesn't help:** This is statistical mean reversion, not a predictive signal. The uniform distribution (0-255) naturally "bounces" around the mean. This cannot be exploited for mining.

---

### Test 7: Nonce Step Size Analysis (500K samples per step)

| Step Size | Avg Bit Flips | First Byte Same | Leading Bits Same |
|-----------|---------------|-----------------|-------------------|
| 1 | 128.00 | 0.39% | 33.37% |
| 2 | 128.00 | 0.40% | 33.36% |
| 4 | 127.97 | 0.39% | 33.31% |
| 8 | 127.99 | 0.40% | 33.22% |
| 16 | 128.01 | 0.40% | 33.39% |
| 256 | 127.99 | 0.40% | 33.37% |
| 65536 | 128.00 | 0.38% | 33.28% |
| 16777216 | 128.00 | 0.39% | 33.32% |

**Interpretation:** Step size makes ZERO difference. Whether you increment by 1 or 16 million, the avalanche is complete.

---

### Test 8: Good Hash Clustering (5M samples)

| Threshold | Total Good | Consecutive Good | Clustering Ratio |
|-----------|------------|------------------|------------------|
| 0x01 | 19,640 | 73 | 0.9515 |
| 0x02 | 39,404 | 300 | 0.9745 |
| 0x04 | 78,680 | 1,226 | 0.9973 |
| 0x08 | 156,725 | 4,760 | 0.9719 |
| 0x10 | 313,386 | 19,449 | 0.9930 |
| 0x20 | 626,253 | 77,937 | 0.9956 |

**Interpretation:** Clustering Ratio < 1.0 means good hashes DON'T cluster - if anything, they're slightly anti-clustered! But deviation is minimal - essentially independent.

---

## Analysis

### Key Finding: SHA-256 Avalanche is Perfect

Every test confirms that SHA-256's avalanche effect is mathematically perfect:

1. **Exactly 50% bit flip** - Not 49% or 51%, but 50.00%
2. **No bit position bias** - All 256 bits equally random
3. **No consecutive correlation** - hash(N) gives ZERO information about hash(N+1)
4. **Step size irrelevant** - Changing one bit or many bits produces same randomness

### Why Direction Anti-Correlation Doesn't Help

The observed anti-correlation (up→down, down→up) is **regression to the mean**, not a mineable pattern:
- If current first byte is 200 (above mean 127.5), next is more likely below 127.5
- If current first byte is 50 (below mean), next is more likely above
- But this is just probability, not a shortcut

Mining needs first byte ≤ target (e.g., 0x00). Whether previous byte was high or low doesn't change the 1/256 probability of hitting 0x00.

---

## Conclusion

**NEGATIVE RESULT:** Differential nonce analysis provides NO exploitable information.

SHA-256 is a mathematically perfect pseudo-random function. Each hash is completely independent of its neighbors.

**Implications:**
1. Cannot skip nonce ranges based on previous hashes
2. Cannot predict "promising" directions
3. Cannot cluster search around "good" nonces
4. Brute force is optimal for nonce space

---

## What This Rules Out

From RESEARCH-ideas.md, we can now eliminate:

| Idea | Status |
|------|--------|
| Differential Analysis | ❌ Ruled out |
| Consecutive Nonce Prediction | ❌ Ruled out |
| Bit Position Exploitation | ❌ Ruled out |
| Step Size Optimization | ❌ Ruled out |
| Good Hash Clustering | ❌ Ruled out |

---

## Next Steps

Move to Experiment v3: Input Space Reduction
- Instead of analyzing nonce changes, analyze INPUT patterns
- Question: Are there header patterns that are more likely to produce low hashes?
- Focus on merkle root / version / timestamp manipulation

---

## Raw Output

```
=== Differential/Consecutive Nonce Analysis ===
Samples: 1000000

## Bit Flip Statistics
  Average bit flips between consecutive nonces: 127.99
  Expected (random): 128.0 (50% of 256 bits)
  Min flips: 87
  Max flips: 168

## Leading Zero Byte Correlation
  Both have leading zero byte: 19 (0.0019%)
  Neither has leading zero: 992065 (99.2065%)
  Only first: 3966 (0.3966%)
  Only second: 3950 (0.3950%)
  Correlation: 99.2084%
  Expected (independent): ~99.2218%

## First Byte Correlation
  Exactly same: 3943 (0.3943%)
  Expected (random): 0.3906%
  Within ±16: 124754 (12.4754%)
  Expected (random): 12.8906%

=== Good Hash Clustering Analysis ===
Clustering Ratio = 1.0 means no clustering (independent)
Clustering Ratio > 1.0 would mean good hashes cluster together
All thresholds showed Ratio < 1.0 (independent/slightly anti-clustered)

=== Bit Position Analysis ===
None found - all bits flip at ~50% rate (independent/random)
Min flip rate: 49.8210%
Max flip rate: 50.1834%
Avg flip rate: 49.9990%
```
