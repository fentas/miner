# TODO - Codebase Audit Fixes

## HIGH Priority

- [x] **Unsafe unwraps on RPC client** (`main.rs`)
  - Fixed: Added proper error handling with `match` and `ok_or_else`

- [x] **Network parse panic** (`stratum/v1.rs:39`)
  - Fixed: Return error instead of panicking on invalid address

- [x] **Silent defaults in block template parsing** (`bitcoin/rpc.rs`)
  - Fixed: Required fields now return proper errors if missing

## MEDIUM Priority

- [x] **Incomplete Bitcoin Core shutdown** (`bitcoin/manager.rs`)
  - Fixed: Added `stop_async()` for graceful RPC shutdown before kill

- [x] **Weak bech32 address decoder** (`main.rs`)
  - Fixed: Added proper BIP-173 checksum validation

- [x] **BIP34 height encoding** (`main.rs`)
  - Reviewed: Already handles all heights correctly (1-4 byte encoding)

## LOW Priority

- [x] **Missing reconnection backoff** (`stratum/v1.rs`)
  - Fixed: Added `connect_with_retry()` with exponential backoff and jitter

- [x] **No config validation** (`config.rs`)
  - Fixed: Added validation for conflicting options (managed+pool preset)
  - Fixed: Added validation for GPU intensity and batch size ranges

- [ ] **No web dashboard authentication** (`web/mod.rs`)
  - Note: Deferred - would require additional dependencies and user configuration

---

## Completed: 2026-01-30
