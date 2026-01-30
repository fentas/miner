# TODO - Code Quality & Test Coverage

## Critical Issues - ALL RESOLVED

- [x] **Weak PRNG for RPC passwords** (`src/bitcoin/manager.rs:1154`)
  - Problem: Uses timestamp-based RNG for generating RPC passwords
  - Fix: Use `getrandom` crate for secure random bytes

- [x] **Silent invalid prev_hash** (`src/stratum/v1.rs:279-283`)
  - Problem: Wrong-length prev_hash silently leaves zeros instead of erroring
  - Fix: Return error if prev_hash is not exactly 32 bytes

- [x] **Silent invalid merkle branches** (`src/stratum/v1.rs:292-302`)
  - Problem: Invalid merkle branches silently skipped
  - Fix: Return error if merkle branch is not exactly 32 bytes

- [x] **Missing port validation** (`src/config.rs`)
  - Problem: No validation that ports are in valid range (1-65535)
  - Fix: Add port range validation and reconnect delay consistency check

## Test Coverage - IMPROVED

- [x] **src/bitcoin/rpc.rs** - Added edge case tests for sync status, block template parsing
- [x] **src/stratum/v1.rs** - Added validation tests for prev_hash, merkle branches
- [x] **src/config.rs** - Added tests for all validation rules
- [x] **src/stats/mod.rs** - Added edge case tests for special values

Total tests: 391 (all passing)

### Remaining Opportunities

- [ ] **src/bitcoin/manager.rs** - Integration tests for download/install flow
- [ ] **src/stratum/v2.rs** - More protocol parsing tests
- [ ] Full Stratum V1 flow integration test
- [ ] Bitcoin Core lifecycle integration test

---

## Completed

### 2026-01-30 (This Session)

- [x] Weak PRNG fix - Using `getrandom` crate for secure random
- [x] Stratum validation - prev_hash and merkle branch length validation
- [x] Config validation - Port ranges, reconnect delays
- [x] Comprehensive test additions - ~50 new tests added

### 2026-01-30 (Previous Session)
- [x] Missing reconnection backoff (`stratum/v1.rs`) - Added `connect_with_retry()`
- [x] No config validation (`config.rs`) - Added validation for conflicting options
- [x] No web dashboard authentication (`web/mod.rs`) - Added optional basic auth
- [x] Windows web directory fix (`main.rs`) - Added `resolve_web_dir()`
- [x] Stratum DoS protection (`stratum/v1.rs`) - Added 64KB size limits
- [x] Thread spawn error handling - Graceful fallback instead of panic
- [x] Atomic ordering optimization - Changed SeqCst to Release/Acquire
- [x] TUI history chart improvements - Filter bar, GPU visibility, borderless
