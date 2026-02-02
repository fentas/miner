# Research Test Log v23: Competitive Edge Analysis

**Date:** 2026-02-02
**Focus:** Net effective hash rate, "freeze rounds" claims, CSA trees, differential patterns
**Goal:** Test if early abort provides NET advantage (speed vs miss rate tradeoff)
**Test file:** `tests/research_v23_test.rs`

---

## Background

The user raises a crucial point:

> "It would not matter if we miss valid hashes as long as we find them faster.
> We are in the realm of never finding any block, so if we could get an edge
> to even find one before others, that's a win."

This reframes the question from "do we miss valid hashes?" to "what's our NET effective hash rate?"

**Formula:** `Net_Effect = Speedup × (1 - Miss_Rate)`

Additional claims to test:
1. **Neutral Bits** - "Freeze first 15-20 rounds" by modifying specific bits
2. **Carry-Save Adders** - Replace standard addition to remove carry delay
3. **Differential "Ghost Patterns"** - Find patterns that survive through rounds

---

## Test Results

### Test 1: Net Effective Hash Rate Analysis

**The Key Question:** Is there ANY abort strategy where NET > 0%?

**Simulation (5M trials, 16-zero target):**

| Abort Round | Threshold | Abort Rate | Speedup | Miss Rate | NET EFFECT |
|-------------|-----------|------------|---------|-----------|------------|
| 40 | A ≤ 0 | 50.0% | 1.10x | **50.6%** | **-45.5%** ✗ |
| 40 | A ≤ 1 | 75.0% | 1.16x | 74.3% | -70.1% ✗ |
| 48 | A ≤ 0 | 50.0% | 1.07x | 48.8% | -45.4% ✗ |
| 48 | A ≤ 1 | 75.0% | 1.10x | 77.9% | -75.6% ✗ |
| 56 | A ≤ 0 | 50.0% | 1.03x | 50.0% | -48.4% ✗ |
| 60 | A ≤ 0 | 50.0% | 1.02x | 49.4% | -48.6% ✗ |

**Critical Finding:** ALL abort strategies produce NET LOSS.

**Why:** Miss rate ≈ abort rate (due to avalanche). The speedup is always less than the miss rate penalty.

---

### Test 2: "Freeze Rounds 15-20" Claim

**Claim:** "Modify only neutral bits → freeze first 15-20 rounds"

**Testing which W[i] modifications freeze which rounds:**

| Modified | R4 frozen? | R8 frozen? | R12 frozen? | R16 frozen? | R20 frozen? |
|----------|------------|------------|-------------|-------------|-------------|
| W[0] | no | no | no | no | no |
| W[3] | no | no | no | no | no |
| W[4] | YES | no | no | no | no |
| W[8] | YES | YES | no | no | no |
| W[12] | YES | YES | YES | no | no |
| W[15] | YES | YES | YES | YES | no |

**Key Observation:** W[i] is frozen through round i-1.

**For Collision Attacks (choose ALL bits):**
- Can choose to modify only W[15]
- Frozen through round 14 → 21.9% savings

**For Mining (nonce at W[3]):**
- MUST modify W[3] (the nonce)
- Frozen through round 2 only → 3.1% savings
- BUT: Midstate already saves 64 rounds (first block)
- Marginal savings: 2/64 = 3.1% of second block only

**Verdict:** The "freeze 15-20 rounds" claim applies to collision attacks, NOT mining.

---

### Test 3: Carry-Save Adder Trees

**This claim is ACTUALLY TRUE.** CSA trees are used in production ASICs.

**Standard Ripple-Carry Adder:**
```
A + B + C + D

Step 1: temp1 = A + B     (wait for 32-bit carry ripple)
Step 2: temp2 = temp1 + C (wait for 32-bit carry ripple)
Step 3: result = temp2 + D (wait for 32-bit carry ripple)

Critical path: 3 × 32 = 96 gate delays
```

**Carry-Save Adder Tree:**
```
Keep Sum and Carry SEPARATE until the very end

CSA(A, B, C) → (Sum1, Carry1)   [parallel, no ripple]
CSA(Sum1, Carry1, D) → (Sum2, Carry2)  [parallel]
...through all 64 rounds...
Final: Sum_final + Carry_final  [one ripple at the end]

Critical path: ~64 XOR gates + 1 final carry propagate
```

**CSA Operation Demo:**
```
A = 0xDEADBEEF
B = 0xCAFEBABE
C = 0x12345678

Traditional A+B+C = 0xBBE0D025
CSA Sum           = 0x06675229
CSA Carry         = 0xB5797DFC
CSA Sum + Carry   = 0xBBE0D025
Match: true ✓
```

**Why This Matters:**
- Each SHA-256 round has 7 additions
- With CSA: additions are parallel XOR/AND gates
- Result: ~2x higher clock speeds possible

**BUT:** This is a HARDWARE optimization, not an algorithmic shortcut.
CSA doesn't reduce the number of rounds - it makes each round faster.
All production ASICs already use CSA trees.

---

### Test 4: Differential "Ghost Pattern" Analysis

**Claim:** "Find a pattern that is 'mostly correct' after 40 rounds"

**Differential Survival Test (1-bit flip in input):**

| Round | Avg Diff (bits) | Min Diff | Max Diff | Survival Rate |
|-------|-----------------|----------|----------|---------------|
| 4 | 87.2 | 59 | 115 | 0.08% |
| 8 | **128.0** | 100 | 160 | **0.00%** |
| 12 | 128.1 | 98 | 162 | 0.00% |
| 16 | 128.0 | 98 | 162 | 0.00% |
| 20 | 127.9 | 96 | 161 | 0.00% |
| 32 | 128.1 | 98 | 159 | 0.00% |
| 40 | 128.1 | 93 | 161 | 0.00% |
| 64 | 128.0 | 90 | 159 | 0.00% |

**Searching for "Lucky" Differentials:**
```
Best single-bit differential: bit 3 with avg 127.6 bits diff at round 16
Expected (random): 128 bits
Improvement: 0.3%
```

**Verdict:** By round 8, differential is ~128 bits (50% = random).
No "ghost pattern" survives past the first few rounds.

---

### Test 5: Mathematical Proof

**For early abort to help, we need:** `m < r × s`

Where:
- m = miss rate (fraction of valid hashes aborted)
- r = abort rate (fraction of nonces aborted)
- s = rounds saved per abort = (64 - R) / 128

**SHA-256's avalanche guarantees:** `m ≈ r`

**Example at round 40:**
```
s = 24/128 = 0.1875 (save 18.75% per abort)
If r = 50%, we need m < 0.5 × 0.1875 = 9.4%
But avalanche guarantees m ≈ r = 50%
Net: (1 - 0.50) / (1 - 0.50 × 0.1875) = 0.50 / 0.906 = 0.55
Result: 45% WORSE than no abort
```

**What would be needed:**
- At round 40: need m < 9.4% while aborting 50%
- This requires: P(valid | not aborted) > 5× P(valid | aborted)
- SHA-256 guarantee: P(valid | any state) = P(valid) exactly

---

### Test 6: Empirical Verification

**10M trials, 12-zero target:**

```
P(valid)                    = 0.023800%
P(valid | looks good at 40) = 0.022942%
P(valid | looks bad at 40)  = 0.024086%

Ratio: 0.9525
If predictive: ratio >> 1
If unpredictive: ratio ≈ 1
```

**Conclusion:** "Looking good" provides NO predictive power.
The ratio is actually slightly < 1, meaning "looking good" is marginally ANTI-predictive.

---

## Analysis: Why the User's Intuition Fails

The user's intuition is correct in THEORY:
> "Missing valid hashes doesn't matter if we find them FASTER"

But it fails in PRACTICE because:

1. **Speedup is modest** - Aborting at round 40 saves 24/128 = 18.75% max
2. **Miss rate equals abort rate** - SHA-256's avalanche guarantees this
3. **Net effect < 1** - Speedup × (1 - miss_rate) = 1.10 × 0.50 = 0.55

**The Formula That Kills Early Abort:**

For abort at round R with rate r:
```
Net = (1 - m) / (1 - r × s)
    ≈ (1 - r) / (1 - r × s)     [because m ≈ r]
    = (1 - r) / (1 - r × s)
```

Since s < 1 (always), the denominator approaches 1 faster than the numerator.
This means Net < 1 for all r > 0.

**The only way early abort could help:**
- If m << r (miss rate much smaller than abort rate)
- This would mean intermediate state predicts validity
- SHA-256's design GUARANTEES this is impossible

---

## Verified Real Optimizations

| Optimization | Status | Speedup |
|--------------|--------|---------|
| Carry-Save Adders | ✓ REAL | ~2x clock speed |
| Midstate | ✓ REAL | 50% fewer rounds |
| ASIC parallelization | ✓ REAL | Linear scaling |
| Pipelining | ✓ REAL | Throughput increase |

---

## Rejected Claimed Shortcuts

| Claim | Status | Why |
|-------|--------|-----|
| Early abort | ✗ IMPOSSIBLE | miss_rate ≈ abort_rate |
| Ghost patterns | ✗ IMPOSSIBLE | No differential survives past round 8 |
| Freeze rounds 15-20 | ✗ MISAPPLIED | Only for collisions, nonce is W[3] |
| Asymptotic matching | ✗ IMPOSSIBLE | No correlation between rounds |

---

## Conclusion

**Competitive Edge Analysis: NEGATIVE (for algorithmic shortcuts)**

| Claim | Test | Result |
|-------|------|--------|
| "Faster even if we miss some" | Net effect calculation | ALL strategies NET LOSE |
| "Freeze 15-20 rounds" | W[i] neutral analysis | Mining: only 3 rounds (W[3]) |
| "CSA trees 2x speed" | Hardware analysis | **TRUE** (already deployed) |
| "Ghost patterns survive" | Differential analysis | 0% survival past round 8 |

**The Fundamental Truth:**
Your intuition ("faster is better") is correct, but SHA-256's avalanche ensures:
- miss_rate ≈ abort_rate
- Net effect = Speedup × (1 - miss_rate) < 1 always

**If early abort worked, SHA-256 would be cryptographically broken.**
This is the DEFINITION of a secure hash function.

---

## Raw Output

```
Net effect at round 40: -45.5% (WORSE than no abort)
Net effect at round 60: -48.6% (WORSE than no abort)
Differential survival at round 8: 0.00%
P(valid | looks good) / P(valid | looks bad) = 0.95 (no predictive power)
CSA trees: Real, ~2x clock speed, already deployed
```
