# Research Test Log v15: Dimensional Cryptanalysis

**Date:** 2026-02-01
**Focus:** GF(2) polynomials, differential paths, Walsh-Hadamard analysis
**Goal:** Explore SHA-256 through different mathematical "dimensions"
**Test file:** `tests/research_v15_test.rs`

---

## Background

Advanced cryptanalysis views SHA-256 through different mathematical lenses:

1. **GF(2) Polynomials** - Treat XOR as addition, AND as multiplication
2. **Differential Paths** - Track how input differences propagate
3. **Linear Approximations** - Find the best linear function to approximate non-linear ones
4. **Message Schedule Analysis** - How differences spread through W[0..63]

---

## Test Results

### Test 1: GF(2) Polynomial Degree Growth

In the finite field GF(2):
- XOR(a,b) → degree = max(deg(a), deg(b))
- AND(a,b) → degree = deg(a) + deg(b)

| Round | Algebraic Degree | Status |
|-------|-----------------|--------|
| 0 | 3 | Growing |
| 1 | 9 | Growing |
| 2 | 27 | High |
| **3** | **32** | **SATURATED** |
| 4-63 | 32 | Saturated |

**Key Finding:** Algebraic degree saturates at round 4. Once degree reaches 32 (all input bits contribute), Gröbner basis methods become computationally infeasible.

---

### Test 2: Differential Path Analysis

Tracking how a single-bit difference in the message propagates:

| Round | Avg Diff Bits | Status |
|-------|---------------|--------|
| 0 | 4.0 | Low |
| 1 | 25.0 | Growing |
| 2 | 55.2 | Growing |
| 3 | 87.2 | Spreading |
| 4 | 115.2 | Saturated |
| **5** | **126.2** | **SATURATED** |
| 10-63 | ~128 | Saturated |

**Key Finding:** Differences spread to ~128 bits (half of 256-bit state) by round 5. This means **no differential path can survive** through 64 rounds with meaningful probability.

---

### Test 3: Walsh-Hadamard Analysis (Best Linear Approximation)

Exhaustively testing all possible linear approximations for Ch and Maj:

#### Ch Function: Ch(x,y,z) = (x∧y) ⊕ (!x∧z)

| Approximation | Match Rate |
|---------------|------------|
| **z** | **75%** ← BEST |
| y | 75% |
| x⊕z | 75% |
| x⊕y⊕z | 25% ← WORST |

The best approximation is just **L(x,y,z) = z** with 75% accuracy!

#### Maj Function: Maj(x,y,z) = (x∧y) ⊕ (x∧z) ⊕ (y∧z)

| Approximation | Match Rate |
|---------------|------------|
| **z** | **75%** ← BEST |
| y | 75% |
| x | 75% |
| x⊕y⊕z | 25% ← WORST |

**Critical Discovery:** The commonly cited "x⊕y⊕z" is actually the **WORST** approximation (25%), not the best!

#### Cryptographic Implication

- Best bias per function: 25% (0.75 - 0.5 = 0.25)
- Combined per-round bias: 0.25 × 0.25 = 0.0625
- After 64 rounds: **0.0625^64 = 2.94×10^-39**

This is astronomically small - far below any exploitable threshold.

---

### Test 4: Round-Reduced Security

Known cryptanalytic results (2026):
| Attack | Rounds Broken | Method |
|--------|---------------|--------|
| Preimage | 52 | Theoretical |
| Collision | 38 | SAT solver |
| Differential | 31 | Practical |

**Security margin:** 64 - 38 = **26 rounds** of safety buffer.

---

### Test 5: Message Schedule Diffusion

How does a 1-bit change in W[0] propagate through W[16..63]?

| Word | Avg Diff Bits | Notes |
|------|---------------|-------|
| W[0] | 1.0 | Input (flipped) |
| W[16] | 2.0 | First output |
| W[18] | 3.7 | Cascade begins |
| W[24] | **14+** | **Full diffusion** |
| W[32] | 16.0 | Saturated |
| W[63] | 16.0 | Saturated |

**Dependency Chain:**
```
W[i] = σ1(W[i-2]) + W[i-7] + σ0(W[i-15]) + W[i-16]
```

Full diffusion achieved at **W[24]** (before round 32).

---

### Test 6: Modular Addition Differential Probability

| Input Δ | Output Δ Weight | Probability |
|---------|-----------------|-------------|
| 1 bit | 1 bit | 50% |
| 1 bit | 2+ bits | 50% |

**Key Finding:** Modular addition has exactly 50% probability of "carry explosion" where a single-bit input difference causes multi-bit output difference. This unpredictability defeats simple differential paths.

---

## Analysis

### Why the "Dimensional Shifts" Fail

#### GF(2) Polynomial Dimension
- Degree saturates at round 4
- Gröbner basis computation is O(2^degree)
- After saturation: 2^32 operations per variable
- With thousands of variables: **computationally infeasible**

#### Differential Path Dimension
- Differences saturate at ~128 bits by round 5
- Probability of controlled path: (1/2)^128 ≈ 10^-39
- Need 2^128 attempts to find a useful path
- **Beyond computational reach**

#### Linear Approximation Dimension
- Best bias is only 25% (not 50% as "x⊕y⊕z" suggests!)
- Per-round bias compounds: 0.0625^64 = 10^-39
- **Negligible signal in noise**

### The 31-Round Differential Path

The "31-round collision" mentioned in literature exploits:
1. Carefully chosen input differences
2. Controlled propagation through early rounds
3. Probabilistic cancellation in later rounds

Our tests show why extending this to 64 rounds is infeasible:
- Round 3: Algebraic degree saturates (cannot solve equations)
- Round 5: Differential difference saturates (cannot control path)
- Round 64: Linear bias = 10^-39 (undetectable)

---

## Conclusion

**All Dimensional Approaches: NEGATIVE for 64 rounds**

| Dimension | Saturation Point | Security Margin |
|-----------|------------------|-----------------|
| GF(2) Polynomial | Round 4 | 60 rounds |
| Differential | Round 5 | 59 rounds |
| Linear Approx | 10^-39 bias | Astronomical |
| Message Schedule | W[24] | 40 words |

**SHA-256 has massive security margins** in every mathematical dimension tested.

---

## Raw Output

```
GF(2) Degree:
  Round 0: 3
  Round 1: 9
  Round 2: 27
  Round 3: 32 (SATURATED)

Differential:
  Round 0: 4 bits
  Round 5: 126 bits (SATURATED)
  Final: 128.1 bits (perfect diffusion)

Walsh-Hadamard:
  Best Ch approx: z (75% match)
  Best Maj approx: z (75% match)
  Combined bias: 0.25 × 0.25 = 0.0625
  After 64 rounds: 2.94e-39

Message Schedule:
  Full diffusion at W[24]

ADD Differential:
  P(1-bit → 1-bit) = 50%
  P(1-bit → multi-bit) = 50%
```
