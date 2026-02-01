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

### Insight #5: Near-Misses Provide Zero Guidance

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v5.md](RESEARCH-test-log-v5.md)*

**Finding:** "Almost good" hashes don't cluster and can't guide search:
- Neighbor analysis: +0.0165 LZ difference (noise)
- Good hash spacing: follows perfect geometric distribution
- Gradient analysis: 83% chance of worsening after improvement (regression to mean)
- Partial match: first byte doesn't predict remaining bytes

**Implication:** There's no "gradient descent" for mining. Near-misses are random - you can't follow them toward valid solutions.

**Gotcha:** If you find a hash with 7 zeros (close to target 8), the next nonce is no more likely to have 8 zeros than any random nonce. Searching "nearby" is futile.

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
| v5: Near-Collision Guided Search | **NEGATIVE** | No clustering, no gradient, no hierarchy. Random is random. |
| v6: Implementation Profiling | **POSITIVE** | Hasher reuse: 2.41x, Memory pattern: 1.79x. Real optimizations exist! |
| v7: Hasher Type Comparison | **MIXED** | All ~23 MH/s. Assembly v1 broken (0.65x). SIMD only 1% faster. |
| v8: Thread Scaling | **POSITIVE** | Near-linear scaling. 8 threads = 9.25x. HT adds 28%. No contention. |

---

## Related Documents

- [RESEARCH-ideas.md](RESEARCH-ideas.md) - Brainstorming and unexplored concepts
- [RESEARCH-test-log-v1.md](RESEARCH-test-log-v1.md) - Truncated rounds experiment
- [RESEARCH-test-log-v2.md](RESEARCH-test-log-v2.md) - Differential nonce analysis
- [RESEARCH-test-log-v3.md](RESEARCH-test-log-v3.md) - Input space reduction
- [RESEARCH-test-log-v4.md](RESEARCH-test-log-v4.md) - Algebraic analysis
- [RESEARCH-test-log-v5.md](RESEARCH-test-log-v5.md) - Near-collision analysis
- [RESEARCH-test-log-v6.md](RESEARCH-test-log-v6.md) - Implementation profiling
- [RESEARCH-test-log-v7.md](RESEARCH-test-log-v7.md) - Hasher type comparison
- [RESEARCH-test-log-v8.md](RESEARCH-test-log-v8.md) - Thread scaling analysis

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

10. **No Gradient Exists** - Mining is not an optimization problem. There's no direction to "descend" toward low hashes. Each nonce is an independent coin flip.

---

## Final Assessment

After v1-v5, we have empirically verified:

### Algorithmic Optimization: EXHAUSTED ❌

**SHA-256 mining algorithm cannot be shortcut.**

- Cannot skip hashes (no mathematical trick)
- Cannot predict winning nonces
- Cannot use partial computation as filter
- Each hash must be fully computed

### Implementation Optimization: OPEN ✓

**SHA-256 implementation can still be improved.**

Just as hardware continues to improve through better engineering, software can optimize:

| Hardware Optimization | Software Equivalent |
|----------------------|---------------------|
| Wider datapaths | Better SIMD utilization |
| Better pipelining | Instruction scheduling |
| Cache hierarchy | Memory access patterns |
| Speculative execution | Branch optimization |
| Parallel ALUs | Instruction-level parallelism |

Areas still to investigate:
1. **Instruction-level parallelism** - Saturating all execution units
2. **SIMD optimization** - AVX-512 vs AVX2 vs multiple streams
3. **Memory prefetching** - Hiding memory latency
4. **Loop unrolling** - Optimal depth for target CPU
5. **Register allocation** - Minimizing spills to memory
6. **Compiler tuning** - Best flags and codegen

---

*"In theory, theory and practice are the same. In practice, they are not."*
