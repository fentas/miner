# Research Test Log v5: Near-Collision and Partial Match Analysis

**Date:** 2026-02-01
**Hypothesis:** "Near misses" might cluster, allowing guided search
**Goal:** Find if partial matches or almost-good hashes can guide the search
**Test file:** `tests/research_v5_test.rs`

---

## Background

The idea: if we find a hash with 7 leading zeros (close to target of 8), maybe nonces nearby are more likely to have 8 zeros. If near-misses cluster, we could search more efficiently by focusing on "hot" regions.

---

## Results

### Test 1: Near-Miss Neighbor Analysis

Samples: 2,000,000

For hashes with ≥8 leading zero bits, checked their neighbors (±1, ±2, ±5, ±10):

| Metric | Value |
|--------|-------|
| Good hashes found | 7,841 |
| Neighbors analyzed | 62,728 |
| Random baseline mean LZ | 0.9990 |
| **Good hash neighbor mean LZ** | **1.0156** |
| Difference | +0.0165 (noise) |
| Neighbors also meeting threshold | 263 (0.42%) |
| Expected rate (random) | 0.39% |

**Interpretation:** Neighbors of good hashes have essentially the same leading zeros as random hashes. No clustering.

---

### Test 2: Progressive Zero Analysis

Samples: 5,000,000

For each leading zero count, what's the mean next nonce's LZ?

| Current LZ | Samples | Mean Next LZ | Diff from Expected |
|------------|---------|--------------|-------------------|
| 0 | 2,498,160 | 0.9995 | -0.0005 |
| 4 | 155,897 | 0.9992 | -0.0008 |
| 8 | 9,809 | 1.0228 | +0.0228 |
| 11 | 1,205 | 1.0307 | +0.0307 |

**Interpretation:** All differences are within noise (~0.03). Having N zeros does NOT make next nonce more likely to have N-1 or N+1 zeros.

---

### Test 3: Partial Match Prediction

Samples: 5,000,000

If first byte is low (0x00), are remaining bytes more likely low?

| First Byte | Samples | Mean Second Byte | Expected |
|------------|---------|------------------|----------|
| 0x00 | 19,495 | 127.55 | 127.5 |
| 0x01 | 19,578 | 127.30 | 127.5 |
| 0x80 | 19,465 | 128.54 | 127.5 |
| 0xFF | 19,412 | 126.95 | 127.5 |

**Interpretation:** All first byte values produce identical second byte distributions. Partial matches don't predict remaining bytes.

---

### Test 4: Good Hash Spacing Analysis

Samples: 10,000,000 (threshold: 8 leading zeros)

| Metric | Observed | Expected (Geometric) |
|--------|----------|---------------------|
| Good hashes found | 39,091 | ~39,062 |
| Mean gap between good hashes | 255.8 | 256.0 |
| Std deviation of gaps | 254.5 | ~256.0 |
| Min gap | 1 | 1 |
| Median gap | 179 | ~177 |
| Small gaps (< 64) | 21.8% | ~22% |

**Interpretation:** Good hashes follow a perfect geometric distribution. No clustering - they're randomly scattered.

---

### Test 5: Multi-Target Analysis

Samples: 2,000,000

If hash meets hard target (6+ zeros), are neighbors more likely to meet easy target (4+ zeros)?

| Metric | Rate |
|--------|------|
| Random hash meets easy target | 6.2424% |
| Expected | 6.2500% |
| Neighbor of hard hash meets easy target | 6.2679% |
| **Difference** | **+0.0254%** (noise) |

**Interpretation:** Finding a hard-target hash doesn't help find easy-target hashes nearby. No hierarchical structure.

---

### Test 6: Hash Gradient Analysis

Samples: 500,000

If LZ is improving (current > previous), does next also improve?

| Metric | Count | Rate |
|--------|-------|------|
| Positive gradient, continued improving | 23,994 | 16.82% |
| Positive gradient, got worse | 118,619 | 83.18% |
| Expected (random) | | 50%/50% |

**Interpretation:** This is regression to the mean! If LZ just improved, it's MORE likely to worsen next (83% vs 17%). No gradient to follow.

---

## Analysis

### Key Finding: Near-Misses Provide Zero Guidance

Every test confirms that "almost good" hashes don't cluster:

1. **No neighbor effect** - Good hash neighbors are random
2. **No progressive pattern** - Current LZ doesn't predict next LZ
3. **No partial match correlation** - First byte doesn't predict second
4. **No clustering** - Good hashes follow geometric distribution
5. **No hierarchy** - Hard targets don't help find easy targets
6. **No gradient** - Can't "follow" improvements toward zeros

### The Regression to Mean Problem

Test 6 shows a striking anti-pattern: if current LZ just improved, next is 83% likely to worsen! This is pure statistics:
- Random walk around mean (~1.0)
- After going up, more likely to go down
- NOT a predictable gradient, just probability

---

## Conclusion

**NEGATIVE RESULT:** Near-collision analysis is NOT viable.

Good hashes are:
1. Randomly distributed in nonce space
2. Independent of each other
3. Unpredictable from partial matches
4. Follow geometric distribution (no clustering)

There is no "gradient descent" or "hot spot" strategy for mining.

---

## What This Rules Out

| Idea | Status |
|------|--------|
| Near-Miss Clustering | ❌ Ruled out |
| Progressive Zero Exploitation | ❌ Ruled out |
| Partial Match Prediction | ❌ Ruled out |
| Good Hash Clustering | ❌ Ruled out |
| Multi-Target Hierarchy | ❌ Ruled out |
| Gradient Descent Mining | ❌ Ruled out |

---

## Overall Research Conclusion

After v1-v5, we have exhaustively tested all major algorithmic optimization paths:

| Experiment | Approach | Result |
|------------|----------|--------|
| v1 | Early rejection via truncated rounds | ❌ NEGATIVE |
| v2 | Consecutive nonce prediction | ❌ NEGATIVE |
| v3 | Input space reduction / nonce bias | ❌ NEGATIVE |
| v4 | Algebraic structure exploitation | ❌ NEGATIVE |
| v5 | Near-collision guided search | ❌ NEGATIVE |

**Final Conclusion:** SHA-256 mining is irreducibly brute force. The only improvements are:
- Faster hardware (ASICs)
- More parallel computation (more chips)
- Quantum computing (theoretical, 10-20+ years away)

Software optimizations are at theoretical maximum.

---

## Raw Output

```
Near-Miss Neighbors:
  Neighbor mean LZ: 1.0156 vs random 0.9990 (diff: +0.0165)

Good Hash Spacing:
  Mean gap: 255.8 (expected: 256.0)
  Std dev: 254.5 (expected: ~256.0)
  Follows geometric distribution perfectly

Hash Gradient:
  If improving, 83% chance of worsening next
  (Regression to mean, not exploitable)
```
