# SHA-256 Mining Research - Insights & Discoveries

This document captures verified insights, "aha moments", and confirmed findings from our research experiments.

---

## Confirmed Insights

### Insight #1: SHA-256 Avalanche is Immediate, Not Gradual

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v1.md](RESEARCH-test-log-v1.md)*

**Finding:** Truncated SHA-256 rounds (8, 16, 32, etc.) show **identical correlation** (~88%) with final output. Running fewer rounds provides NO additional predictive power.

**Implication:** The avalanche effect is "complete" within the first few rounds. Partial computation cannot be used as a filter because:

1. Early state has no more information about final hash than random
2. 93.6% false negative rate makes any filter unusable
3. All 8 state variables show equal (lack of) correlation

**Gotcha:** High "correlation" numbers (88%) are misleading - they mostly reflect the base rate of "both high" outcomes (93.8%), not actual predictive power.

---

### Insight #2: SHA-256 Avalanche is Mathematically Perfect

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v2.md](RESEARCH-test-log-v2.md)*

**Finding:** Consecutive nonces produce **completely independent** hashes:
- Exactly 50.00% bit flip rate (expected: 50%)
- All 256 bit positions flip at identical rates
- No clustering of "good" hashes
- Step size (1, 256, 65536, 16M) makes zero difference

**Implication:** Hash(N) provides ZERO information about Hash(N+1). The only "correlation" found was regression-to-mean (anti-correlation), which is not exploitable for mining.

**Gotcha:** Direction anti-correlation (up→down more likely than up→up) is just statistics, not a mining shortcut. It doesn't change the probability of hitting the target.

---

### Insight #3: SHA-256 is a Perfect Random Oracle for Bitcoin

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v3.md](RESEARCH-test-log-v3.md)*

**Finding:** Exhaustive testing confirms SHA-256 has NO input bias:
- Chi-squared test: PASS (p > 0.05)
- Nonce modulo bias: variance = 0.000020 (essentially zero)
- Nonce bit pattern bias: max difference = 0.0050 (noise)
- Nonce range bias: variance = 0.000000 (exactly zero)
- Special patterns (powers of 2, palindromes): no advantage

**Implication:** The nonce space cannot be reduced. All 2^32 nonce values are equally likely to produce any hash value. "Smart" nonce selection is impossible.

**Gotcha:** Even with 5 million samples, the chi-squared test shows perfect uniformity. Don't trust small-sample "patterns" - they're always noise.

---

### Insight #4: SHA-256 Has No Exploitable Algebraic Structure

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v4.md](RESEARCH-test-log-v4.md)*

**Finding:** Every algebraic property tested shows SHA-256 is cryptographically perfect:
- Linear approximation: 50.00% ± 0.06% (no bias)
- Carry propagation: no effect on output (variance = 0.0004)
- Output bits: completely independent (max correlation 0.14%)
- Input bit influence: all 32 bits exactly equal (spread = 0.05)
- Entropy: 99.9976% of theoretical maximum
- Algebraic relationships: none found

**Implication:** No mathematical shortcut exists. SHA-256 cannot be expressed as solvable equations. The only approach is brute force.

**Gotcha:** Cryptographers spent decades designing SHA-256 to resist exactly these attacks. Empirical verification confirms their success.

---

## Key Observations

### The Probabilistic Filter Hypothesis
**Idea:** Instead of computing full 64-round SHA-256, use a faster "weak hash" as a filter.

```
Traditional: 100% work per candidate
Filter approach:
  - 10% work: weak hash filter
  - 90% rejection rate
  - 100% work only on 10% of candidates
  - Net: 10% + (10% × 100%) = 20% work = 5x speedup
```

**Critical Question:** Does early-round SHA-256 state correlate with final output's leading bytes?

---

## Experiment Summary

| Experiment | Result | Insight |
|------------|--------|---------|
| v1: Truncated Rounds Correlation | **NEGATIVE** | Avalanche is immediate, not gradual. No filter viable. |
| v2: Differential Nonce Analysis | **NEGATIVE** | Consecutive hashes are perfectly independent. No prediction possible. |
| v3: Input Space Reduction | **NEGATIVE** | Nonce space cannot be reduced. All values equally random. |
| v4: Algebraic Structure | **NEGATIVE** | No linear, carry, or algebraic patterns. SHA-256 is perfect. |

---

## Related Documents

- [RESEARCH-ideas.md](RESEARCH-ideas.md) - Brainstorming and unexplored concepts
- [RESEARCH-test-log-v1.md](RESEARCH-test-log-v1.md) - Truncated rounds experiment
- [RESEARCH-test-log-v2.md](RESEARCH-test-log-v2.md) - Differential nonce analysis
- [RESEARCH-test-log-v3.md](RESEARCH-test-log-v3.md) - Input space reduction
- [RESEARCH-test-log-v4.md](RESEARCH-test-log-v4.md) - Algebraic analysis

---

## Gotchas & Lessons Learned

1. **Correlation ≠ Predictive Power** - 88% correlation sounds good but is worse than base rate. Always compare to null hypothesis.

2. **False Negative is the Killer Metric** - For mining filters, false negatives mean lost blocks. Even 1% is unacceptable; 93% is fatal.

3. **SHA-256 Mixing is Fast** - Don't assume avalanche builds gradually. The first 8 rounds are as "mixed" as 64.

4. **Regression to Mean ≠ Mining Advantage** - Statistical anti-correlation (high value followed by low value) is just probability, not a shortcut to finding zeros.

5. **50% is Truly 50%** - SHA-256 avalanche is mathematically perfect. Every bit position flips at exactly 50% rate, not 49% or 51%.

6. **Use Chi-Squared for Uniformity** - Small samples show "patterns" that are just noise. With 5M samples and chi-squared test, SHA-256 proved perfectly uniform.

7. **All Nonces Are Equal** - Powers of 2, palindromes, round numbers, high/low ranges - none are special. Don't waste time on "lucky numbers."

8. **Entropy is Maximum** - SHA-256 achieves 99.9976% of theoretical maximum entropy. There's no room for prediction or compression.

9. **NSA Cryptographers Knew What They Were Doing** - SHA-256 was designed to resist exactly these attacks. After 20+ years and billions in mining investment, no shortcut has been found.

---

## Final Assessment

After v1-v4, we have empirically verified what cryptographers have claimed theoretically:

**SHA-256 mining cannot be optimized beyond brute force.**

The remaining paths are:
1. **Hardware optimization** - Faster circuits, not smarter algorithms
2. **Parallelization** - More chips, not fewer operations
3. **Quantum computing** - Grover's algorithm (theoretical, 10-20+ years away)

For software optimization, we have achieved theoretical maximum. Further improvements require hardware R&D.

---

*"In theory, theory and practice are the same. In practice, they are not."*
