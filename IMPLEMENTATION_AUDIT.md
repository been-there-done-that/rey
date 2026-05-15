# Implementation Audit Report

**Date**: 2026-05-15
**Branch**: audit/implementation-vs-plan
**Commit**: a2f37ae (Implement thumbnail crate)

---

## Executive Summary

The Rey project has **6 of 12 crates implemented** (Layer 0 and Layer 1 complete), while **Layer 2 and Layer 3 crates remain stubs** with only placeholder `fn add(a, b)` functions. The workspace compiles cleanly. Of 33 tests run, 31 pass and 2 fail (environment variable tests in `common`).

**Completion: ~50% by crate count, ~80% by lines of code**

---

## 1. Compilation Status

| Check | Status |
|---|---|
| `cargo check --workspace` | PASS |
| `cargo test --workspace` | FAIL (2/33 tests failing) |
| `cargo clippy --workspace` | Not yet run |
| `cargo fmt --check` | Not yet run |

### Failing Tests
- `common::config::tests::test_from_env_reads_db_path` — env var `REY_DB_PATH` not being read correctly
- `common::config::tests::test_from_env_reads_log_level` — env var `REY_LOG_LEVEL` not being read correctly

---

## 2. Crate-by-Crate Implementation Status

### Layer 0 — Foundation (COMPLETE)

#### `types` — 100% Complete (+1 extra file)
| Planned File | Status | Lines | Tests |
|---|---|---|---|
| `crypto.rs` | Implemented | 163 | 8 |
| `file.rs` | Implemented | 143 | 3 |
| `collection.rs` | Implemented | 58 | 2 |
| `sync.rs` | Implemented | 86 | 3 |
| `upload.rs` | Implemented | 143 | 5 |
| `device.rs` | Implemented | 68 | 3 |
| `share.rs` | Implemented | 54 | 2 |
| `user.rs` | Implemented | 110 | 4 |
| `error.rs` | Implemented | 71 | 3 |
| `sse.rs` | Extra (not in plan) | 107 | 4 |

**Total**: ~873 lines, 37 tests. All planned modules present.

#### `common` — 100% Complete
| Planned File | Status | Lines | Tests |
|---|---|---|---|
| `config.rs` | Implemented (2 tests failing) | 215 | 8 |
| `error.rs` | Implemented | 95 | 4 |
| `telemetry.rs` | Implemented | 60 | 3 |
| `time.rs` | Implemented | 91 | 7 |
| `result.rs` | Implemented | 94 | 4 |

**Total**: ~565 lines, 26 tests. All planned modules present.

**Issues**: 2 failing tests related to environment variable reading in `config.rs`.

---

### Layer 1 — Pure Libraries (MOSTLY COMPLETE)

#### `crypto` — 100% Complete (+1 extra file)
| Planned File | Status | Lines | Tests |
|---|---|---|---|
| `aead/secretbox.rs` | Implemented | 57 | 3 |
| `aead/stream.rs` | Implemented | 66 | 4 |
| `kdf/argon.rs` | Implemented | 77 | 4 |
| `kdf/blake2b.rs` | Implemented | 76 | 5 |
| `key/generate.rs` | Implemented | 27 | 2 |
| `key/encrypt.rs` | Implemented | 24 | 1 |
| `key/decrypt.rs` | Implemented | 37 | 2 |
| `seal/box_.rs` | Implemented | 102 | 5 |
| `seal/keypair.rs` | Implemented | 27 | 2 |
| `util.rs` | Implemented | 93 | 8 |
| `error.rs` | Implemented | 34 | 0 |
| `prop_tests.rs` | Extra (not in plan) | 43 | 4 |

**Total**: ~664 lines, 46 tests. All planned modules present. `#![no_std]` compatible with feature flags.

#### `image` — 100% Complete (+1 extra file)
| Planned File | Status | Lines | Tests |
|---|---|---|---|
| `decode.rs` | Implemented | 65 | 5 |
| `encode.rs` | Implemented | 33 | 2 |
| `resize.rs` | Implemented | 55 | 4 |
| `exif.rs` | Implemented | 119 | 1 |
| `orientation.rs` | Implemented | 67 | 5 |
| `error.rs` | Extra (not in plan) | 11 | 0 |

**Total**: ~363 lines, 17 tests. All planned modules present.

#### `metadata` — 80% Complete (missing `magic.rs`)
| Planned File | Status | Lines | Tests |
|---|---|---|---|
| `encrypt.rs` | Implemented | 12 | 0 |
| `decrypt.rs` | Implemented | 13 | 0 |
| `structs.rs` | Implemented (re-exports types) | 1 | 0 |
| `magic.rs` | **MISSING** | — | — |
| `error.rs` | Extra (not in plan) | 11 | 0 |

**Total**: ~206 lines, 9 tests (inline in lib.rs). Missing `magic.rs` for server-side sorting metadata.

#### `thumbnail` — 90% Complete (missing `preview.rs`, +3 extras)
| Planned File | Status | Lines | Tests |
|---|---|---|---|
| `generate.rs` | Implemented | 84 | 4 |
| `encrypt.rs` | Implemented | 5 | 0 |
| `decrypt.rs` | Implemented | 12 | 0 |
| `cache/mod.rs` | Implemented | 162 | 1 |
| `cache/memory.rs` | Implemented | 88 | 4 |
| `cache/disk.rs` | Implemented | 212 | 4 |
| `download.rs` | Implemented | 17 | 0 |
| `preview.rs` | **MISSING** | — | — |
| `inflight.rs` | Extra (not in plan) | 83 | 3 |
| `invalidation.rs` | Extra (not in plan) | 13 | 0 |
| `error.rs` | Extra (not in plan) | 17 | 0 |

**Total**: ~691 lines, 16 tests. Missing `preview.rs` for grid preview generation.

---

### Layer 2 — Application Logic (ALL STUBS)

#### `local-db` — 0% Complete (STUB)
| Planned File | Status |
|---|---|
| `connection.rs` | **MISSING** |
| `migrations/` | **MISSING** |
| `collections.rs` | **MISSING** |
| `files.rs` | **MISSING** |
| `sync_state.rs` | **MISSING** |
| `search.rs` | **MISSING** |

**Current**: 14 lines, 1 trivial test (`fn add(2, 2)`). Cargo.toml has NO dependencies.

#### `sync` — 0% Complete (STUB)
| Planned File | Status |
|---|---|
| `pull.rs` | **MISSING** |
| `diff.rs` | **MISSING** |
| `decrypt.rs` | **MISSING** |
| `thumbnails.rs` | **MISSING** |
| `cursor.rs` | **MISSING** |

**Current**: 14 lines, 1 trivial test. Cargo.toml has NO dependencies.

#### `zoo-client` — 0% Complete (STUB)
| Planned File | Status |
|---|---|
| `orchestrator.rs` | **MISSING** |
| `upload.rs` | **MISSING** |
| `download.rs` | **MISSING** |
| `sse.rs` | **MISSING** |
| `types.rs` | **MISSING** |

**Current**: 14 lines, 1 trivial test. Cargo.toml has NO dependencies.

#### `zoo` (server) — 0% Complete (STUB)
| Planned File | Status |
|---|---|
| `config.rs` | **MISSING** |
| `types.rs` | **MISSING** |
| `state.rs` | **MISSING** |
| `db/` | **MISSING** |
| `s3/` | **MISSING** |
| `sse/` | **MISSING** |
| `workers/` | **MISSING** |
| `api/` | **MISSING** |
| `auth/` | **MISSING** |
| `bin/zoo-server.rs` | **MISSING** |
| `migrations/` | **MISSING** |

**Current**: 14 lines, 1 trivial test. Cargo.toml has NO dependencies.

---

### Layer 3 — Platform Bindings (ALL STUBS)

#### `client-lib` — 0% Complete (STUB)
| Planned File | Status |
|---|---|
| `commands/mod.rs` | **MISSING** |
| `commands/auth.rs` | **MISSING** |
| `commands/files.rs` | **MISSING** |
| `commands/collections.rs` | **MISSING** |
| `commands/sync.rs` | **MISSING** |
| `commands/upload.rs` | **MISSING** |
| `commands/thumbnails.rs` | **MISSING** |
| `commands/device.rs` | **MISSING** |
| `commands/search.rs` | **MISSING** |
| `state.rs` | **MISSING** |

**Current**: 14 lines, 1 trivial test. Cargo.toml has NO dependencies.

#### `zoo-wasm` — 0% Complete (STUB)
**Current**: 14 lines, 1 trivial test. Cargo.toml has NO dependencies.

---

## 3. Dependency Graph Verification

| Crate | Planned Deps | Actual Deps | Status |
|---|---|---|---|
| `types` | serde, serde_json | serde, serde_json, zeroize | OK (zeroize addition) |
| `common` | types, tracing, anyhow, thiserror | types, serde, serde_json, thiserror, tracing, tracing-subscriber, dirs-next, toml | OK (anyhow missing, extras added) |
| `crypto` | types, aead, chacha20poly1305, xsalsa20poly1305, x25519-dalek, argon2, blake2b_simd, rand_core | Matches + thiserror, base64, hex, subtle | OK |
| `image` | types, image, exif | types, common, image, kamadak-exif, thiserror, chrono | OK |
| `metadata` | types, crypto, image | types, crypto, serde_json, thiserror | MISSING image dep |
| `thumbnail` | types, crypto, image, metadata | types, crypto, rey-image, lru, dashmap, tokio, thiserror, base64, serde, serde_json | MISSING metadata dep |
| `local-db` | types, common, rusqlite | **EMPTY** | NOT STARTED |
| `sync` | types, crypto, metadata, thumbnail, local-db, common | **EMPTY** | NOT STARTED |
| `zoo-client` | types, reqwest | **EMPTY** | NOT STARTED |
| `zoo` | types, common, axum, tokio, sqlx, aws-sdk-s3, tower | **EMPTY** | NOT STARTED |
| `client-lib` | sync, local-db, thumbnail, zoo-client, tauri | **EMPTY** | NOT STARTED |
| `zoo-wasm` | zoo-client, wasm-bindgen | **EMPTY** | NOT STARTED |

**Layer integrity**: Lower layers never depend on higher layers. Confirmed.

---

## 4. Test Coverage Analysis

| Crate | Meaningful Tests | Stub Tests | Coverage Quality |
|---|---|---|---|
| `types` | 37 | 0 | Good — serde round-trips, serialization, enum variants |
| `common` | 26 (2 failing) | 0 | Good — config, error, time, telemetry |
| `crypto` | 46 | 0 | Excellent — includes property tests |
| `image` | 17 | 0 | Good — decode, encode, resize, EXIF, orientation |
| `metadata` | 9 | 0 | Good — round-trip tests in lib.rs |
| `thumbnail` | 16 | 0 | Good — generate, cache, memory, disk, inflight |
| `local-db` | 0 | 1 | None — stub only |
| `sync` | 0 | 1 | None — stub only |
| `zoo-client` | 0 | 1 | None — stub only |
| `zoo` | 0 | 1 | None — stub only |
| `client-lib` | 0 | 1 | None — stub only |
| `zoo-wasm` | 0 | 1 | None — stub only |

**Total**: 151 meaningful tests, 6 stub tests.

---

## 5. Missing vs Spec Requirements

### Requirements Covered by Implemented Crates
- Req 25 (Crate Architecture) — Workspace structure correct
- Req 1-6 (Crypto, Key Management) — Fully implemented
- Req 10-11 (Thumbnail) — Mostly implemented (missing preview)
- Req 26 (EXIF) — Implemented
- Req 24.3, 24.4 (Zero-knowledge compile-time) — Dependency graph correct

### Requirements NOT Covered (Stub Crates)
- Req 7-8 (Incremental Sync) — `sync` crate is a stub
- Req 9 (Local SQLite DB) — `local-db` crate is a stub
- Req 12-19 (Upload Service/Zoo) — `zoo` crate is a stub
- Req 20 (File Download) — `zoo` crate is a stub
- Req 21 (Search) — `local-db` crate is a stub
- Req 22 (Tauri Desktop) — `client-lib` crate is a stub
- Req 23 (Web Platform) — `zoo-wasm` crate is a stub
- Req 27-28 (Upload Cancellation/Queue) — `zoo`/`zoo-client` are stubs

---

## 6. Action Items (Priority Order)

### P0 — Fix Existing Issues
1. **Fix 2 failing tests in `common`**: `config.rs` env var reading for `REY_DB_PATH` and `REY_LOG_LEVEL`
2. **Add `metadata` dependency to `thumbnail/Cargo.toml`** (planned but missing)
3. **Add `image` dependency to `metadata/Cargo.toml`** (planned but missing)

### P1 — Complete Missing Layer 1 Files
4. **Implement `metadata/magic.rs`** — Public magic metadata for server-side sorting
5. **Implement `thumbnail/preview.rs`** — Grid preview generation

### P2 — Implement Layer 2 (Application Logic)
6. **Implement `local-db`** — SQLite with SQLCipher, migrations, CRUD, FTS5 search
7. **Implement `zoo-client`** — Upload/download state machine, SSE client
8. **Implement `sync`** — Incremental diff sync, cursor tracking, batch decrypt
9. **Implement `zoo`** — Full server: auth, uploads, downloads, SSE, workers, API

### P3 — Implement Layer 3 (Platform Bindings)
10. **Implement `client-lib`** — Tauri commands, AppState
11. **Implement `zoo-wasm`** — WASM bindings for web

### P4 — Testing & Quality
12. **Add integration tests** for local-db (temp SQLite), sync (wiremock), zoo-client (wiremock)
13. **Add property tests** where specified in tasks.md
14. **Run `cargo clippy --workspace -- -D warnings`** and fix all warnings
15. **Run `cargo fmt --check`** and format

---

## 7. Risk Assessment

| Risk | Impact | Mitigation |
|---|---|---|
| `local-db` has no Cargo.toml deps | Blocks sync, client-lib | Add rusqlite with sqlcipher feature |
| `sync` has no Cargo.toml deps | Blocks client-lib | Add all planned deps |
| `zoo` has no Cargo.toml deps | Largest single crate, blocks nothing directly | Add axum, sqlx, aws-sdk-s3, etc. |
| 2 failing tests in common | CI failure | Fix env var parsing |
| No integration tests for Layer 2 | Quality gap | Plan wiremock/SQLx test infrastructure |

---

## 8. Estimated Remaining Work

| Phase | Crates | Estimated Effort |
|---|---|---|
| P0 (Fixes) | common, thumbnail, metadata | 1-2 hours |
| P1 (Missing Layer 1) | metadata, thumbnail | 2-4 hours |
| P2 (Layer 2) | local-db, zoo-client, sync, zoo | 40-80 hours |
| P3 (Layer 3) | client-lib, zoo-wasm | 10-20 hours |
| P4 (Testing) | All crates | 10-20 hours |

**Total estimated remaining**: 63-126 hours
