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

---

## Related Documents

- [RESEARCH-ideas.md](RESEARCH-ideas.md) - Brainstorming and unexplored concepts
- [RESEARCH-test-log-v1.md](RESEARCH-test-log-v1.md) - Truncated rounds experiment

---

## Gotchas & Lessons Learned

1. **Correlation ≠ Predictive Power** - 88% correlation sounds good but is worse than base rate. Always compare to null hypothesis.

2. **False Negative is the Killer Metric** - For mining filters, false negatives mean lost blocks. Even 1% is unacceptable; 93% is fatal.

3. **SHA-256 Mixing is Fast** - Don't assume avalanche builds gradually. The first 8 rounds are as "mixed" as 64.

---

*"In theory, theory and practice are the same. In practice, they are not."*
