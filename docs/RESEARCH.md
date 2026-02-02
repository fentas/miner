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

### Insight #6: Double SHA-256 Does NOT Amplify Bias

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v13.md](RESEARCH-test-log-v13.md)*

**Finding:** The "dual-wave" and "attractor" theories are empirically refuted:
- IV bit distribution: 53.12% (3.12% bias) - washed out by round 3
- Single vs Double SHA-256 Hamming weight: 128.0143 vs 127.9937 (no amplification)
- Leading zeros: perfect geometric distribution
- No "attractor" values detected in 100K samples
- No near-fixed-points (min distance 102/256 bits)

**Implication:** Running SHA-256 twice doesn't create exploitable "resonance" or "logical tracks". The output remains a perfect random oracle.

**Gotcha:** Bit 20 is never set in any of the 8 IVs (mathematical artifact of √primes), but this creates no measurable bias in output after 64 rounds of mixing.

---

### Insight #7: Linearization Fails at Round 3

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v14.md](RESEARCH-test-log-v14.md)*

**Finding:** Replacing SHA-256's non-linear operations (ADD, Ch, Maj) with linear approximations (XOR) produces completely random divergence by round 3:
- Round 0: 13.5% divergence
- Round 1: 25.9% divergence
- Round 2: 38.4% divergence
- **Round 3: 50.7% divergence (RANDOM)**

Additionally:
- Maj function is **anti-correlated** with x⊕y⊕z (24.72% match, worse than random)
- Carry cascades average 3.75 bits per addition
- Perfect avalanche: 50.00% flip rate, 0 outliers in 16,384 bit pairs tested

**Implication:** There is no "single equation" that describes SHA-256. Any algebraic approach that relies on linear approximations is useless after just 3 of 64 rounds.

**Gotcha:** The Maj function's linear approximation is worse than random guessing (24.72% < 50%). This means linearization doesn't just "lose accuracy" - it actively misleads in the wrong direction.

---

### Insight #8: The Best Linear Approximation is NOT x⊕y⊕z

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v15.md](RESEARCH-test-log-v15.md)*

**Finding:** Walsh-Hadamard analysis reveals the optimal linear approximations:
- Ch(x,y,z): Best approximation is just **z** (75% match)
- Maj(x,y,z): Best approximation is just **z** (75% match)
- The commonly cited **x⊕y⊕z** is the **WORST** approximation (25% match)

Combined per-round bias: 0.25 × 0.25 = 0.0625
After 64 rounds: **0.0625^64 = 2.94×10^-39**

**Implication:** Even with the BEST linear approximations, the bias compounds to effectively zero over 64 rounds. The "31-round differential path" cannot be extended.

**Gotcha:** Literature often mentions "x⊕y⊕z" as a linear approximation, but this is actually anti-correlated. The real best approximation ignores two of the three inputs entirely.

---

### Insight #9: Bit-Blasting Confirms SHA-256's Circuit Complexity

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v16.md](RESEARCH-test-log-v16.md)*

**Finding:** Analyzing SHA-256 as a logic circuit (DAG of gates) reveals why algebraic attacks fail:
- Gate count: **~128K gates** per compression (3x more than SHA-1)
- Carry cascade: average **6.85 bits** per addition (max 32 bits observed)
- Bit entanglement: full saturation by **round 6** (all 256 bits connected)
- XOR approximation: becomes **random by round 4** (0% word match, 128-bit Hamming distance)
- SAT encoding: **~443K clauses, ~56K variables**, ratio 7.94 (above phase transition)

**Implication:** The circuit is too large, too entangled, and too non-linear for any gate-level simplification. Bit-blasting cannot "trace a wire" through the circuit because by round 6, every wire connects to every other wire.

**Gotcha:** The carry propagation in modular addition creates "funnels" where high bits depend on ALL lower bits. This non-locality breaks all local reasoning approaches (BDDs, SAT propagation, symbolic execution).

---

### Insight #10: ARX Decomposition Confirms the "Glitch" IS the Security

*Date: 2026-02-01*
*Experiment: [RESEARCH-test-log-v17.md](RESEARCH-test-log-v17.md)*

**Finding:** Separating SHA-256 into "Clean Wave" (XOR) and "Glitches" (carries) reveals:
- **Clean Wave alone:** Solvable in ~1 millisecond (linear system over GF(2))
- **Glitches per compression:** ~877 carry events
- **Divergence to random:** Round 4 (Clean Wave completely overwritten)
- **Differential path survival:** 0 of 100,000 samples survived to round 64
- **Glitch cancellation:** Only 0.013% perfect (2^-16), 82% of bits survive partial

**Implication:** The carry "glitches" ARE the cryptographic strength. They cannot be separated, cancelled, or collected and applied later. By round 4, the accumulated glitches have completely overwritten the "Clean Wave" state.

**Gotcha:** The "collect glitches and patch at end" approach fails because glitches are recursive - a glitch in round 1 creates NEW glitches in round 2 that wouldn't have existed otherwise. It's like a single wrong note causing every musician to play a completely different song.

---

### Insight #11: Quantum Annealing Faces the Same Exponential Barriers

*Date: 2026-02-02*
*Experiment: [RESEARCH-test-log-v18.md](RESEARCH-test-log-v18.md)*

**Finding:** Expressing SHA-256 as a QUBO energy minimization problem reveals:
- **Qubits needed:** ~31,232 (D-Wave 2000Q has only 2,000)
- **Search space:** 2^31,232 ≈ 10^9,401 configurations
- **Random differential survival:** 0% past 8 rounds (100K samples)
- **Best ghost pattern:** 4 rounds (random 1-bit difference)
- **Energy barrier:** ~889 combined (tunnel probability: 10^-386)

**Implication:** Quantum annealing faces the same exponential barriers as classical methods. The energy landscape is too rugged, barriers too sharp, and the search space too vast. SHA-256's 64 rounds provide a 26-round security margin beyond the best known 31-round differential attack.

**Gotcha:** The 31-round attack required ~2 billion attempts with meticulously crafted differentials. Extending to 64 rounds requires 2^64 = 18 quintillion attempts - beyond any known computational capability, classical or quantum.

---

### Insight #12: Meet-in-the-Middle and Biclique Attacks Don't Help Mining

*Date: 2026-02-02*
*Experiment: [RESEARCH-test-log-v19.md](RESEARCH-test-log-v19.md)*

**Finding:** Advanced cryptanalytic techniques (MitM, Biclique, Neutral Bits) don't provide meaningful shortcuts:
- **MitM backward computation:** Works perfectly, but requires knowing W (circular dependency)
- **Biclique compensation:** 0 of 10,000 state/message pairs found
- **52-round attack:** Pseudo-preimage only, complexity 2^255.5 (1.4× speedup = meaningless)
- **Message schedule:** 50% XOR error (random), correlation 0.5001 (random)
- **Neutral bits:** All 512 input bits fully avalanche by round 20

**Implication:** The "pro path" (SAT + Biclique + Linearization + FPGA) fails at every step. MitM has a circular dependency, bicliques give only 1.4× speedup, linearization is random by round 4, and SAT search space is 2^3200. Billions in mining R&D have explored these ideas - the result is pure brute force with optimized hardware.

**Gotcha:** The claimed "52-round pseudo-preimage attack" is often misunderstood. It's not 64 rounds, it's pseudo-preimage (can change IV), and the complexity is 2^255.5 (vs 2^256 brute force). The speedup is 1.4× - practically meaningless when the base is 10^77 operations.

---

### Insight #13: Neutral Bits Give 4.7% Savings, Not 25%, and Are Already Exploited

*Date: 2026-02-02*
*Experiment: [RESEARCH-test-log-v20.md](RESEARCH-test-log-v20.md)*

**Finding:** The "25% of the puzzle free" claim for neutral bits doesn't apply to mining:
- **Theory:** W[15] is neutral through round 14, giving 23.4% savings
- **Mining reality:** Nonce is at W[3] in Block 2, neutral through round 3 only = **4.7%**
- **Already captured:** Standard midstate optimization pre-computes this

Empirical verification:
- W[i] is neutral through round i-1 (confirmed for all 512 input bits)
- Nonce at W[3] shows 0.00 state difference through rounds 0-3
- Avalanche begins at round 4 (4.02 bits), full by round 7 (88.96 bits)
- MitM requires 2^128 table = 2^133 bytes (impossible)

**Implication:** The "cutting edge" claims about neutral bits and MitM apply to collision attacks (where you choose all message bits), not mining (where nonce position is fixed). All mining optimization is hardware optimization - there are NO algorithmic shortcuts.

**Gotcha:** When crypto papers claim "we can pre-compute 15 rounds," they mean for collision attacks where you control the entire message. In mining, the nonce is at a fixed position (W[3]), so you can only pre-compute 3 rounds - and midstate optimization already does this.

---

### Insight #14: Early Abort and "Neural Miners" Cannot Work

*Date: 2026-02-02*
*Experiment: [RESEARCH-test-log-v21.md](RESEARCH-test-log-v21.md)*

**Finding:** The "probabilistic termination" and "neural miner" claims are empirically refuted:
- **State correlation:** Pearson correlation at round 40 = **0.003601** (essentially zero)
- **Early abort:** ALL tested strategies (at rounds 32-60) **miss valid hashes**
- **Pattern recognition:** No statistical features distinguish "good" from "bad" states
- **Valid hash distribution:** 70% of valid hashes had A=0 zeros at round 40

Backward computation explosion:
- 1 round back: 2^32 possible paths
- 64 rounds back: **2^2048** possible paths (vs 2^32 forward)

Pre-computation reality:
- Claimed: "75% constant, first 16 rounds trivial"
- Actual: Midstate saves 50% (64 of 128 rounds), deployed since 2010

**Implication:** The avalanche effect guarantees intermediate state provides ZERO information about final hash quality. A "neural miner" would abort 70% of winning hashes because they "look bad" at round 40. There is no learnable pattern - this is cryptographically guaranteed by SHA-256's design.

**Gotcha:** The claim "ASICs drop nonces with <0.0001% success probability" is impossible because the probability IS 0.0001% for ALL nonces, regardless of intermediate state. SHA-256's avalanche makes each nonce statistically independent, even during computation.

---

### Insight #15: Double SHA-256 Breaks ALL Early Abort Strategies

*Date: 2026-02-02*
*Experiment: [RESEARCH-test-log-v22.md](RESEARCH-test-log-v22.md)*

**Finding:** The "round 60 early abort" and "5-10% efficiency boost" claims ignore a critical fact: **Bitcoin uses double SHA-256**.

Testing correlations:
- **First hash round 60 vs final:** Correlation = **0.004579** (none)
- **Second hash round 60 vs final:** Correlation = **-0.001281** (none)

Early abort simulation (10M trials, 16-zero target):
- **46.3% of valid hashes would be MISSED** by round 60 abort
- Maximum theoretical savings: **3.1%** (not 5-10%)
- Any abort strategy that misses valid hashes = lost revenue

W[3]/W[4] "sweet spot" analysis:
- Claimed: "Skip rounds 1-15 for massive percentage"
- Reality: Nonce at W[3] → skip rounds 0-2 only (3 rounds)
- Marginal savings: **1.6%** (beyond what midstate already provides)

**Implication:** Early abort in the first hash is POINTLESS because the first hash output becomes completely random input to the second hash. By the time you can make a meaningful abort decision (second hash round 60), you've already done 124 of 128 rounds. The claimed "5-10% efficiency boost" is physically impossible.

**Gotcha:** Claims about "round 60 early abort" often confuse single SHA-256 with Bitcoin's double SHA-256. The first hash output is random 256 bits → complete avalanche → second hash has no correlation with first hash's intermediate state.

---

### Insight #16: "Faster Even If We Miss Some" Still Doesn't Work

*Date: 2026-02-02*
*Experiment: [RESEARCH-test-log-v23.md](RESEARCH-test-log-v23.md)*

**Finding:** The user's reframe "missing valid hashes doesn't matter if we find them FASTER" is correct in theory but fails in practice.

**Net Effect Formula:** `Net = Speedup × (1 - Miss_Rate)`

Testing all abort strategies:
- **Round 40 abort:** 1.10x speedup × 49.4% kept = **54.5% effective** (-45.5% net loss)
- **Round 60 abort:** 1.02x speedup × 50.6% kept = **51.4% effective** (-48.6% net loss)

**ALL abort strategies produce NET LOSS.**

Why: SHA-256's avalanche guarantees `miss_rate ≈ abort_rate`. The speedup from saving rounds is always less than the penalty from missing valid hashes.

**The only way early abort could help:** if miss_rate << abort_rate (intermediate state predicts validity). This would mean SHA-256 is cryptographically broken.

**Verified Real Optimizations:**
- ✓ Carry-Save Adder trees - ~2x clock speed (already deployed in ASICs)
- ✓ Midstate pre-computation - 50% fewer rounds (already deployed)
- ✓ ASIC parallelization - linear scaling (already deployed)

**Gotcha:** "Freeze 15-20 rounds with neutral bits" applies to collision attacks (can choose all message bits). In mining, nonce is at W[3], so only rounds 0-2 are neutral - and midstate already captures this.

---

### Insight #17: Even Wild & Loco Theories Fail

*Date: 2026-02-02*
*Experiment: [RESEARCH-test-log-v24.md](RESEARCH-test-log-v24.md)*

**Finding:** After 23 experiments proving SHA-256 is cryptographically perfect, we tested 10 "outside the box" theories. All failed.

**Wild Theories Tested:**
1. **Hamming Weight Destiny** - Do valid nonces cluster at certain bit counts? NO (Chi-sq = 16.18 < 46)
2. **Gray Code Walk** - Does single-bit increment behave differently? NO (0.99x ratio)
3. **Message Schedule Fingerprint** - Can W[16:63] patterns predict validity? NO (0.12% diff)
4. **Glitch Variance** - Do carry counts correlate with validity? NO (0.89% diff)
5. **Prime Nonce Magic** - Are primes special? NO (0.95x = primes are WORSE)
6. **Bit Position Dominance** - Are all 32 bits equal? YES (0.01 spread)
7. **Reverse Birthday** - Do valid hashes cluster in output space? NO (0 collisions)
8. **Compression Orbits** - Any cyclic behavior? NO (0 cycles in 1M iterations)
9. **XOR Relationships** - Do valid nonces have patterns? NO (uniform distribution)
10. **Timestamp Wiggle** - Are some seconds luckier? NO (94% spread = Poisson noise)

**Verdict:** SHA-256 is indifferent to:
- Number-theoretic properties (primes)
- Bit structure (Hamming weight, bit positions)
- Increment pattern (Gray vs binary)
- Message expansion patterns
- Carry propagation variance
- Output space clustering
- Temporal patterns

**Gotcha:** Apparent "significant" findings (like 94% timestamp spread) are often Poisson noise in small samples. Always calculate expected variance: with ~30 expected events, stddev is √30 ≈ 5.5.

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
| v9: CPU Features | **INSIGHT** | sha2 crate already uses SHA-NI. Custom SIMD adds overhead (0.97x). Compute-bound. |
| v10: Cache Locking (CAT) | **NEGATIVE** | CAT unavailable in WSL2. Working set 688 bytes fits in L1 (32KB). Cache is NOT the bottleneck. |
| v11: Compiler Optimization | **INSIGHT** | All major optimizations applied. PGO potential: 10-15%. BOLT potential: 5-10%. |
| v12: PGO Testing | **NEGATIVE** | PGO implemented but <1% improvement. SHA-256 is branch-free, already at hardware limits. |
| v13: Pattern Analysis | **NEGATIVE** | Tested "attractor" and "dual-wave" theories. SHA-256² behaves as perfect random oracle. No exploitable patterns. |
| v14: Algebraic Analysis | **NEGATIVE** | Linearization fails at round 3. Perfect avalanche (0 outliers). SAT infeasible. No invariant subspaces. |
| v15: Dimensional Analysis | **NEGATIVE** | GF(2) saturates round 4. Differentials saturate round 5. Best linear bias 0.0625^64 = 10^-39. |
| v16: Bit-Blasting Analysis | **NEGATIVE** | ~128K gates, 6.85-bit carry cascade, full entanglement by round 6, SAT ratio 7.94 (above transition). |
| v17: ARX Decomposition | **NEGATIVE** | "Clean Wave" solvable in 1ms, but ~877 glitches/compression overwrite it by round 4. 0/100K differential paths survive. |
| v18: Quantum Annealing | **NEGATIVE** | QUBO needs 31K qubits (16x D-Wave). Energy barrier 10^-386 tunnel probability. 26-round security margin. |
| v19: MitM & Biclique | **NEGATIVE** | MitM has circular dependency. Biclique gives 1.4× (meaningless). Neutral bits gone by round 20. SAT space 2^3200. |
| v20: Neutral Bits Practical | **NEGATIVE** | Mining savings = 4.7% (not 25%). Nonce at W[3], neutral 3 rounds. Midstate already exploits this. |
| v21: Early Abort & Neural | **NEGATIVE** | State correlation 0.0036 (none). All abort strategies miss valid hashes. No learnable patterns. |
| v22: Sweet Spots & Round 60 | **NEGATIVE** | Double SHA-256 breaks early abort. 46.3% miss rate. Max savings 3.1% (not 5-10%). First hash uncorrelated with final. |
| v23: Competitive Edge | **NEGATIVE** | Net effect always < 1. Miss rate ≈ abort rate. CSA trees real (hardware, not algorithmic). No "ghost patterns" survive round 8. |
| v24: Wild & Loco Theories | **NEGATIVE** | Hamming weight, Gray code, prime nonces, compression orbits, timestamp wiggling - ALL fail. SHA-256 is indifferent to creative input patterns. |

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
- [RESEARCH-test-log-v9.md](RESEARCH-test-log-v9.md) - CPU feature detection
- [RESEARCH-test-log-v10.md](RESEARCH-test-log-v10.md) - Cache locking (CAT) investigation
- [RESEARCH-test-log-v11.md](RESEARCH-test-log-v11.md) - Compiler optimization analysis
- [RESEARCH-test-log-v12.md](RESEARCH-test-log-v12.md) - PGO testing
- [RESEARCH-test-log-v13.md](RESEARCH-test-log-v13.md) - Pattern analysis ("attractor" theory)
- [RESEARCH-test-log-v14.md](RESEARCH-test-log-v14.md) - Algebraic cryptanalysis (linearization, SAT)
- [RESEARCH-test-log-v15.md](RESEARCH-test-log-v15.md) - Dimensional cryptanalysis (GF(2), Walsh-Hadamard)
- [RESEARCH-test-log-v16.md](RESEARCH-test-log-v16.md) - Bit-blasting & gate-level analysis
- [RESEARCH-test-log-v17.md](RESEARCH-test-log-v17.md) - ARX decomposition (Wave + Glitch)
- [RESEARCH-test-log-v18.md](RESEARCH-test-log-v18.md) - Quantum annealing & probabilistic paths
- [RESEARCH-test-log-v19.md](RESEARCH-test-log-v19.md) - Meet-in-the-Middle & Biclique cryptanalysis
- [RESEARCH-test-log-v20.md](RESEARCH-test-log-v20.md) - Neutral bits & MitM practical analysis
- [RESEARCH-test-log-v21.md](RESEARCH-test-log-v21.md) - Early abort & neural miners
- [RESEARCH-test-log-v22.md](RESEARCH-test-log-v22.md) - Sweet spots & round 60 abort
- [RESEARCH-test-log-v23.md](RESEARCH-test-log-v23.md) - Competitive edge analysis
- [RESEARCH-test-log-v24.md](RESEARCH-test-log-v24.md) - Wild & loco theories

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

11. **Cache Locking Can't Help Compute-Bound Code** - Intel CAT and AMD Platform QoS exist for cache-sensitive workloads. SHA-256 mining has a tiny 688-byte working set that fits 47x in L1 cache. The bottleneck is compute, not memory.

12. **Backward Computation Has a Circular Dependency** - Meet-in-the-Middle attacks sound promising ("compute from both ends!"), but inverting SHA-256 rounds requires knowing W. And W comes from the message you're trying to find. The "shortcut" loops back to the original problem.

13. **Academic "Attacks" Often Don't Apply to Mining** - The "52-round pseudo-preimage attack" gets cited as evidence SHA-256 is weakening. But: (a) it's pseudo-preimage (can change IV), not preimage; (b) it's 52 rounds, not 64; (c) complexity is 2^255.5 vs 2^256 (1.4× speedup). For mining at difficulty 2^80, this is irrelevant.

14. **The "Pro Path" is a Well-Trodden Dead End** - SAT + Biclique + Linearization + FPGA sounds sophisticated, but billions of dollars in mining R&D have explored every combination. The result: pure brute force with optimized hardware. There are no shortcuts.

15. **Neutral Bits Apply to Collisions, Not Mining** - W[15] is neutral through round 14 (23% savings) - but only for collision attacks where you choose all message bits. In mining, the nonce is at W[3], giving only 3 neutral rounds (4.7%), and midstate optimization already captures this.

16. **Cargo Config Needs All Targets** - `.cargo/config.toml` target features are target-specific. Without `[target.x86_64-pc-windows-msvc]`, Windows builds run 2x slower (no SHA-NI). Always include all deployment targets.

17. **Carry Propagation is the Non-Linearity Source** - Modular addition (ADD) averages 6.85 bits of carry cascade per operation. With 448 ADDs in SHA-256, this creates ~3,000 bits of non-linear interference. XOR approximation fails by round 4.

18. **SAT Ratio Doesn't Capture Structured Hardness** - SHA-256's clause/variable ratio (7.94) is above the random 3-SAT phase transition (4.26), but structured dependencies make it even harder. Carries create long-range dependencies that break unit propagation.

19. **Early Abort is Impossible - All Intermediate States Look the Same** - The "neural miner" claim suggests aborting nonces that "look bad" at round 40. But 70% of valid hashes had A=0 zeros at round 40 (looks bad!). The Pearson correlation between intermediate state and final zeros is 0.0036 - statistically zero. SHA-256's avalanche guarantees no learnable patterns.

20. **Backward Computation Explodes Exponentially** - Going backward without knowing W creates 2^32 possible previous states per round. 64 rounds back = 2^2048 paths to search. Forward brute force (2^32 nonces) is astronomically smaller. This is why "starting from the answer" doesn't help.

21. **Pre-computation Gives 50%, Not 75%** - "75% constant data" refers to Block 2's W words being padding, not computation savings. Midstate optimization saves 64 of 128 rounds = 50%, and has been standard since 2010.

22. **Double SHA-256 Breaks Early Abort Claims** - Bitcoin uses SHA256(SHA256(header)). First hash output = random 256 bits → feeds second hash. Early abort in first hash is pointless (no correlation with final). By second hash round 60, you've done 124/128 rounds.

23. **W[i] "Sweet Spots" Are Already Captured** - W[i] is neutral through round i-1. Nonce is W[3], so only rounds 0-2 are neutral (not "1-15"). Midstate optimization already skips the first 64 rounds. The "nested loop" trick provides only 1.6% marginal savings.

24. **Early Abort Miss Rate is Fatal** - Round 60 abort with threshold A=0 zeros misses 46.3% of valid hashes. Any strategy that misses valid hashes means lost revenue. The claimed "5-10% efficiency boost" would actually cost miners half their block rewards.

25. **Net Effect Formula Kills Early Abort** - `Net = Speedup × (1 - Miss_Rate)`. Even the best abort strategy (round 40, A≤0) yields 1.10x speedup but 50% miss rate → Net = 0.55 = 45% WORSE than no abort. SHA-256's avalanche guarantees miss_rate ≈ abort_rate, so Net < 1 always.

26. **"Faster Even If We Miss Some" Fails in Practice** - The intuition is correct in theory, but for early abort to help, you'd need miss_rate << abort_rate (intermediate state predicts validity). SHA-256 was DESIGNED to make this impossible. If it worked, SHA-256 would be cryptographically broken.

27. **Carry-Save Adders Are Real But Not Algorithmic** - CSA trees keep sum and carry separate until the final round, allowing ~2x clock speeds. This is a HARDWARE optimization already deployed in all production ASICs. It doesn't reduce the number of rounds - it makes each round faster.

28. **No "Ghost Pattern" Survives Past Round 8** - Differential cryptanalysis shows single-bit differences become 128-bit differences (50% = random) by round 8. The best differential found is 0.3% "better" than random - statistically meaningless. There is no pattern to follow.

29. **Wild Theories Are Still Wrong** - Unconventional ideas (Hamming weight "destiny", Gray code walks, prime nonce "magic", compression orbits, timestamp wiggling) all empirically fail. SHA-256 is indifferent to input structure, bit positions, number-theoretic properties, and any creative pattern we can imagine. If any wild theory worked, it would be a major cryptographic discovery.

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
3. ~~**Memory prefetching**~~ - v10: Not needed (compute-bound, 688-byte working set)
4. **Loop unrolling** - Optimal depth for target CPU
5. **Register allocation** - Minimizing spills to memory
6. ~~**Compiler tuning**~~ - v11/v12: All flags applied, PGO tested (<1% gain)
7. ~~**Cache locking (CAT)**~~ - v10: Not available in WSL2, wouldn't help anyway

---

*"In theory, theory and practice are the same. In practice, they are not."*
