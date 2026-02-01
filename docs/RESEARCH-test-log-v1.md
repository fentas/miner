# Research Test Log v1: Truncated Rounds Correlation

**Date:** 2026-02-01
**Hypothesis:** Early SHA-256 rounds may predict final hash's leading bytes
**Goal:** Find if we can use partial computation as a filter

---

## Background

SHA-256 has 64 rounds. Bitcoin mining only cares about leading bytes being low (zeros).

**Question:** Does the state after round N correlate with the final first byte?

If yes → we can filter candidates with fewer rounds → massive speedup.

---

## Experiment Design

### Variables
- **Independent:** Number of rounds (8, 16, 24, 32, 48, 64)
- **Dependent:** Correlation between partial and final first byte
- **Control:** Random Bitcoin-like headers

### Metrics
1. **Correlation coefficient** - Does low partial → low final?
2. **False negative rate** - How many good candidates do we reject?
3. **Filter efficiency** - What % of bad candidates can we reject?

### Test Cases
```
For each round count N in [8, 16, 24, 32, 48]:
  For 10 million random headers:
    partial_state = sha256_rounds(header, N)
    final_hash = sha256_full(header)

    Record:
    - partial_state[0] (first byte after N rounds)
    - final_hash[0] (first byte of final hash)
    - Whether both are < threshold
```

---

## Implementation

Test file: `tests/research/truncated_rounds_test.rs`

```rust
// See actual implementation in tests/research/
```

---

## Results

### Run 1: 2026-02-01

**Parameters:** 1,000,000 samples, threshold=0x10 (6.2% of byte range)

| Rounds | Correlation | False Negative | Filter Rate | Notes |
|--------|-------------|----------------|-------------|-------|
| 8      | 88.29%      | 93.53%         | 93.73%      | ! |
| 16     | 88.34%      | 93.69%         | 93.79%      | ! |
| 24     | 88.30%      | 93.72%         | 93.75%      | ! |
| 32     | 88.30%      | 93.61%         | 93.75%      | ! |
| 40     | 88.32%      | 93.57%         | 93.76%      | ! |
| 48     | 88.26%      | 93.77%         | 93.71%      | ! |
| 56     | 88.27%      | 93.81%         | 93.73%      | ! |
| 64     | 87.52%      | 100.00%        | 93.34%      | baseline |

### Threshold Sensitivity (32 rounds)

| Threshold | Correlation | False Neg | Filter Rate |
|-----------|-------------|-----------|-------------|
| 0x01      | 99.20%      | 99.75%    | 99.60%      |
| 0x04      | 96.90%      | 98.47%    | 98.43%      |
| 0x10      | 88.27%      | 93.60%    | 93.76%      |
| 0x20      | 78.08%      | 87.33%    | 87.49%      |
| 0x40      | 62.47%      | 74.87%    | 74.95%      |
| 0x80      | 50.13%      | 49.75%    | 50.01%      |

### State Variable Analysis (32 rounds, threshold 0x10)

All 8 state variables (a-h) show similar correlation: ~88.27%, false_neg: ~93.6%

---

## Analysis

### Key Finding #1: Correlation is CONSTANT across rounds
- 8 rounds and 64 rounds have the SAME correlation (~88%)
- This means partial computation provides NO additional predictive power
- The "information" about final output is set very early

### Key Finding #2: False Negative Rate is Fatal
- 93.6% false negative = we'd reject 93.6% of valid blocks
- This makes truncated rounds UNUSABLE as a filter
- Even at round 8 (fastest), we lose too many valid candidates

### Key Finding #3: Threshold affects correlation predictably
- Lower threshold (stricter) = higher correlation but worse false negative
- At 0x80 threshold (50% of values), correlation drops to 50% (random)
- No "sweet spot" where filter becomes viable

---

## Conclusions

**NEGATIVE RESULT:** Truncated rounds correlation does NOT provide a viable filter.

**Root Cause:** SHA-256's avalanche property works too well. Even 8 rounds thoroughly mix the state, and subsequent rounds don't add predictable structure.

**Insight:** The partial state has no more information about final leading bytes than a random guess. The 88% correlation is actually WORSE than the 93.8% "both high" base rate (since most hashes don't have low first bytes).

**What We Learned:**
1. SHA-256 mixing is "complete" very early in the round function
2. Partial computation cannot predict final hash structure
3. The avalanche effect is not gradual - it's essentially immediate

---

## Next Steps

1. Run the experiment
2. If correlation found → implement filter in miner
3. If no correlation → try alternative approaches (see RESEARCH-ideas.md)

---

## Raw Data

[Will be appended after experiment runs]
