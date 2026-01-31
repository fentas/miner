# Test Coverage Documentation

This document describes the test coverage for the BTC Miner project.

## Overview

**Total Tests: 500+**
**Line Coverage: 50.45%** (core mining modules)

The codebase has extensive test coverage focusing on:
- Unit tests for core functionality
- Integration tests with mock servers
- Edge case handling
- Concurrent access testing
- Block finding and submission paths

## Test Distribution by Module

### Source Code Unit Tests

| Module | Tests | Coverage Focus |
|--------|-------|----------------|
| `src/config.rs` | 38 | Config parsing, defaults, validation, all presets |
| `src/tui/state.rs` | 37 | TUI state management, getters/setters |
| `src/bitcoin/manager.rs` | 35 | Bitcoin Core management, download, paths |
| `src/mining/sha256_simd.rs` | 27 | SHA256 SIMD implementations |
| `src/mining/hasher.rs` | 27 | Hashing operations, midstate, all hasher types |
| `src/mining/job.rs` | 26 | Job creation, header building, targets, varint encoding |
| `src/mining/mod.rs` | 22 | Coordinator, stats tracking, GPU hashes |
| `src/main.rs` | 21 | Main app flow tests |
| `src/mining/gpu_worker.rs` | 16 | GPU job creation, coordinator |
| `src/web/mod.rs` | 13 | Web API endpoints |
| `src/stratum/v1.rs` | 13 | Stratum V1 protocol |
| `src/stats/mod.rs` | 13 | Statistics tracking |
| `src/idle/mod.rs` | 13 | Idle detection |
| `src/stratum/v2.rs` | 11 | Stratum V2 protocol |
| `src/bitcoin/rpc.rs` | 11 | RPC client |
| `src/tui/widgets.rs` | 9 | UI widget rendering |
| `src/gpu/opencl.rs` | 9 | OpenCL GPU mining |
| `src/gpu/manager.rs` | 7 | GPU device management |
| `src/stratum/mod.rs` | 5 | Stratum abstractions |
| `src/mining/worker.rs` | 5 | CPU worker threads |
| `src/bitcoin/mod.rs` | 3 | Bitcoin types |

### Integration Tests

| Test File | Tests | Coverage Focus |
|-----------|-------|----------------|
| `tests/integration_tests.rs` | 34 | End-to-end mining flow, block submission, GPU coordination |
| `tests/block_finding_tests.rs` | 23 | Share struct, block building, target comparison, hasher consistency |
| `tests/gpu_tests.rs` | 21 | GPU coordinator, job handling, tuning params |
| `tests/stratum_integration_tests.rs` | 18 | Mock stratum server, protocol flow |
| `tests/mining_tests.rs` | 15 | Mining primitives, encoding |
| `tests/rpc_integration_tests.rs` | 11 | Mock RPC server tests |
| `tests/config_tests.rs` | 7 | Config file parsing |
| `tests/web_api_tests.rs` | 19 | HTTP API endpoints |
| `tests/tui_tests.rs` | 1 | TUI state |

## Critical Path Coverage

### Block Submission Flow (Solo Mining)
- Job creation from block template
- Header construction with merkle root
- Varint encoding for transaction counts
- Block building with coinbase + transactions
- Target comparison (pool vs network)
- Block submission via RPC
- **Tests:** `test_job_build_block_*`, `test_block_*`, `test_varint_*`

### Share Submission Flow (Pool Mining)
- Stratum subscribe/authorize
- Job notification parsing
- Difficulty tracking
- Share construction and submission
- Accept/reject handling
- **Tests:** `test_stratum_*`, `test_share_*`

### RPC Communication
- Connection establishment
- Authentication
- Block template fetching
- Block submission
- Error handling (warmup, IBD, no peers)
- **Tests:** `test_mock_*` in `rpc_integration_tests.rs`

### Mining Coordination
- CPU worker thread management
- GPU worker coordination
- Job distribution
- Nonce range allocation
- Stats aggregation
- **Tests:** `test_coordinator_*`, `test_cpu_gpu_*`

### Idle Mode
- Environment detection (Linux, WSL2, Windows)
- Timeout handling
- State transitions
- CPU/GPU selective control
- **Tests:** `test_idle_*`, `test_environment_*`

## Excluded from Coverage

The following are intentionally excluded from coverage metrics:

### TUI/UI Code (`#[cfg(not(tarpaulin_include))]`)
- `src/tui/widgets.rs` - Terminal rendering
- `src/tui/mod.rs` - TUI main loop
- Visual display functions

### Platform-Specific Code
- Windows-only FFI (`GetLastInputInfo`)
- Linux-only X11 calls (`xprintidle`)
- GPU kernel code (OpenCL)

### Interactive Features
- Keyboard handling
- Real-time display updates

## Running Tests

```bash
# Run all tests
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test file
cargo test --test integration_tests

# Run tests matching pattern
cargo test block_submission
```

## Coverage Tools

To generate coverage reports:

```bash
# Install tarpaulin
cargo install cargo-tarpaulin

# Run coverage (excluding TUI)
cargo tarpaulin --exclude-files 'src/tui/*' --out Html
```

## Adding Tests

When adding new functionality:

1. **Unit tests** - Add in the same file under `#[cfg(test)]` module
2. **Integration tests** - Add to appropriate file in `tests/`
3. **Mock tests** - Use `wiremock` for HTTP/RPC, TCP listener for stratum

### Test Naming Convention
- `test_<module>_<functionality>` for unit tests
- `test_<flow>_<scenario>` for integration tests
- `test_mock_<rpc_method>_<scenario>` for RPC tests

## Coverage Goals

| Category | Target | Current | Notes |
|----------|--------|---------|-------|
| Core Mining | 95%+ | 68% | hasher.rs: 68%, job.rs: 78% |
| Block Submission | 95%+ | 78% | job.rs build_block paths |
| RPC Client | 90%+ | 15% | Async calls need mock server |
| Stratum Protocol | 90%+ | 97% | v1.rs: 97%, v2.rs: 100% |
| Config Parsing | 95%+ | 86% | config.rs: 86% |
| Stats/Metrics | 90%+ | 100% | stats/mod.rs: 100% |
| GPU Coordination | 85%+ | 71% | gpu_worker.rs: 71% |
| Idle Detection | 80%+ | 31% | Platform-specific code |
| TUI/UI | N/A | Excluded | Visual rendering code |
| SHA256 SIMD | 80%+ | 85%+ | avx2: 92%, avx512: 93%, generic: 96% |

## Recent Additions

### Block Finding Tests (`tests/block_finding_tests.rs`)
- `test_share_has_required_fields` - Share struct validation
- `test_share_ntime_encoding_for_pool` - ntime big-endian encoding
- `test_share_nonce_encoding_for_pool` - nonce little-endian encoding
- `test_share_extranonce2_encoding_for_pool` - extranonce2 hex encoding
- `test_job_has_solo_mining_fields` - Solo mining job requirements
- `test_job_build_block_missing_coinbase` - Error handling
- `test_job_build_block_missing_transactions` - Error handling
- `test_job_meets_network_target` - Block detection
- `test_hash_meets_target_comparison` - Target comparison logic
- `test_difficulty_to_target_values` - Difficulty conversion
- `test_job_build_header_structure` - Header byte layout
- `test_block_varint_encoding` - Bitcoin varint 1/3/5/9 byte formats
- `test_all_hashers_share_detection` - All hasher types produce same hash
- `test_hash_nonce_range_finds_share` - Nonce range search
- `test_coordinator_share_channel` - Share channel functionality

### Hasher Type Tests
- `test_hasher_sha2_type` - Sha2Crate hasher path
- `test_hasher_simd_type` - SIMD hasher path
- `test_hasher_asm_type` - Assembly hasher path
- `test_hasher_asm_v2_type` - Assembly V2 hasher path
- `test_hasher_asm_v3_type` - Assembly V3 hasher path
- `test_all_hasher_types_produce_same_result` - Hash consistency across types

### Stats Tests
- `test_stats_gpu_hashes` - GPU hash counter
- `test_padded_atomic_operations` - Cache-line padded atomics
- `test_padded_atomic_default` - Default initialization

### RPC Mock Tests
- `test_mock_getblocktemplate` - Template parsing
- `test_mock_getblocktemplate_warmup` - Warmup error
- `test_mock_getblocktemplate_no_peers` - Peer error
- `test_mock_getblocktemplate_ibd` - IBD error
- `test_mock_getconnectioncount` - Peer count
- `test_mock_submitblock_stale` - Stale block rejection
