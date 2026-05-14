# Rey Architecture Document

## Context

Rey is an image encryption application where encryption happens exclusively on the user device and the server only coordinates — stores ciphertext, manages metadata, facilitates sharing, and enforces access policy. The server never sees plaintext or keys.

This document specifies the crate and project organization only. No implementation details.

---

## 1. The Rust Component: Cargo Workspace

### 1.1 Virtual Manifest

The workspace root `Cargo.toml` contains only a `[workspace]` section with no `[package]`. This is a **virtual manifest** (the pattern used by rust-analyzer, Tokio, Bevy, and Tauri itself for projects between 10K and 1M+ LOC).

Rationale: A virtual manifest avoids polluting the root with `src/`, keeps `cargo build` from defaulting to a single package, and enforces explicit `-p` flags or `default-members`. Every crate lives one level deep under `crates/`.

### 1.2 Crate Layout

```
crates/
├── types/              # Layer 0 — shared types, serde only
├── common/             # Layer 0 — config, error types, telemetry
├── crypto/             # Layer 1 — AEAD, key derivation, key management
├── image/              # Layer 1 — image decode/encode, resize, EXIF
├── metadata/           # Layer 1 — encrypt/decrypt file metadata
├── thumbnail/          # Layer 1 — thumbnail pipeline, generation, cache
├── local-db/           # Layer 2 — client-side SQLite (decrypted metadata)
├── sync/               # Layer 2 — incremental diff sync, cursor tracking
├── zoo-client/         # Layer 2 — client SDK, no I/O, no crypto
├── zoo/                # Layer 2 — monolithic server (internal modules)
├── client-lib/         # Layer 3 — Tauri command layer (desktop)
└── zoo-wasm/           # Layer 3 — WASM bindings (web)
```

### 1.3 Dependency Flow

```
types              (zero deps — only serde, no internal deps)
    ↑
common             (depends on: types)
crypto             (depends on: types)
image              (depends on: types)
    ↑
metadata           (depends on: types, crypto, image)
thumbnail          (depends on: types, crypto, image)
zoo                (depends on: types, common)
zoo-client         (depends on: types)
    ↑
local-db           (depends on: types, common)
sync               (depends on: types, crypto, metadata, thumbnail, local-db, common)
    ↑
client-lib         (depends on: sync, local-db, thumbnail, zoo-client)
zoo-wasm           (depends on: zoo-client)
```

Key rule: Lower layers never depend on higher layers. `crypto` cannot import `zoo`. Cargo enforces this at compile time — the dependency simply isn't listed in `Cargo.toml`.

### 1.4 Crate Responsibilities

| Crate | Role | Ships in |
|---|---|---|---|
| `types` | Wire formats, request/response structs, enums, `serde` derive | client + server |
| `common` | Config loading, unified error types, tracing/logging setup | client + server |
| `crypto` | AEAD (XChaCha20-Poly1305 / XSalsa20-Poly1305), Argon2 key derivation, nonce generation | client only |
| `image` | PNG/JPEG/WebP decode/encode, resize, EXIF extraction | client only |
| `metadata` | Encrypt/decrypt file metadata structs, magic metadata | client only |
| `thumbnail` | Thumbnail generation, encrypt/decrypt, disk + memory cache | client only |
| `local-db` | Client-side SQLite, decrypted metadata CRUD, search | client only |
| `sync` | Incremental diff sync from Zoo, cursor tracking, batch decrypt | client only |
| `zoo-client` | Upload/download state machine, SSE listener, no crypto | client only |
| `zoo` | Monolithic server: API routes, DB, S3, SSE, auth, workers, sharing | server only |
| `client-lib` | Tauri command layer, wires sync + local-db + thumbnail | desktop binary |
| `zoo-wasm` | WASM bindings for zoo-client | web WASM |

### 1.5 Rust Features That Enable This Architecture

- **Cargo workspace**: Single `Cargo.lock` across client and server means identical dependency resolution. Shared `target/` avoids redundant compilation.

- **`[workspace.dependencies]`** (Rust 1.64+): All dependency versions defined once in root. Every crate inherits. No version drift.

- **`[workspace.package]`**: Shared metadata (license, edition) inherited by all crates.

- **Feature flags prevent cross-contamination**: `client-lib` never links axum or sqlx. The dependency graph is physically separated at compile time. A compromised server cannot leak crypto primitives it was never compiled with.

- **`[profile.dev.package.*]`**: Crypto crate can be compiled at `opt-level = 3` even in debug builds, keeping encryption fast during development while server crates stay debug-friendly.

- **Parallel compilation**: `crypto` and `image` have no internal dependency on each other — Cargo compiles them in parallel. `zoo` is fully independent of both and compiles concurrently. Wide, shallow graph = fast incremental builds.

- **Crate boundaries as compilation units**: Change only `client-lib`? Only `client-lib` and its reverse deps recompile. `zoo` is untouched.

- **No circular dependencies**: Cargo refuses to build a crate graph with cycles. This forces clean layering from day one.

- **`xtask` pattern** (optional): Dev automation (codegen, benchmark runner, key generation scripts) in a dedicated `crates/xtask/` crate instead of shell scripts. Same language, same toolchain, no Makefile rot.

- **`no_std` pathway for crypto**: `crypto` and `types` can be written with `#![no_std]` + `alloc` support, feature-gated. If the need ever arises to encrypt on an embedded/IoT device, these crates compile without `std`.

---

## 2. The Non-Rust Components

The application spans these platform targets:

| Target | Purpose | Phase |
|---|---|---|
| **Desktop** (macOS, Windows, Linux) | Primary native experience | Phase 1 |
| **Web** | Lightweight access, sharing, management | Phase 1 |
| **Mobile** (iOS, Android) | On-device image capture and encryption | Deferred |

### 2.1 Phased Platform Strategy

#### Phase 1 (Now): Web + Tauri Desktop

Ship web and desktop first. Both share the same frontend framework and Rust backend.

```
┌──────────────────────────────────────┐
│    Shared Frontend (web tech)        │
│    React / Svelte / Solid / Vue      │
│                                      │
│    ┌──────────┐  ┌────────────────┐  │
│    │ Web App  │  │ Tauri Desktop  │  │
│    │ (Vite)   │  │ (same UI code) │  │
│    └────┬─────┘  └───────┬────────┘  │
│         │                │           │
│         │ invoke()       │           │
│         │ (WASM)         │ (IPC)     │
│         ▼                ▼           │
│    ┌──────────┐  ┌────────────────┐  │
│    │ WASM     │  │  Rust Backend  │  │
│    │ zoo-wasm │  │  client-lib    │  │
│    │ crypto   │  │  (Tauri)       │  │
│    └──────────┘  └────────────────┘  │
└──────────────────────────────────────┘
```

Key properties:
- **One frontend codebase** — web app also bundles as Tauri's WebView renderer
- **Shared UI components** via `packages/ui`
- **Web**: Encryption via WASM (`crypto` compiled to WASM) + `zoo-client` for upload/download
- **Desktop**: Encryption via native Rust (`crypto` compiled to native) via Tauri IPC
- **Distribution**: Web via Vercel/static host; Desktop via `.app`, `.dmg`, `.deb`, `.rpm`, `.exe`, `.msi`, AppImage

This avoids committing to a mobile architecture until the product-market fit is validated. The Rust crate boundaries are designed so that adding mobile later requires no changes to `crypto`, `image`, `metadata`, `thumbnail`, `zoo-client`, or `zoo` — only a new platform binding.

#### Phase 2 (Future): Mobile (Native or Tauri)

Two options when mobile is needed:

**Option A — Tauri Mobile** (if it matures enough):
Same frontend codebase, Tauri's iOS/Android runtime. No new UI code. Rust backend runs on-device. Requires Swift/Kotlin plugins for camera, keychain, biometrics.

**Option B — Native** (Kotlin Android + Swift iOS):
Dedicated platform UIs. Rust shared via UniFFI. Maximum platform fidelity. Higher maintenance.

The decision is deferred. When it's time, the existing crate boundaries make either path viable without re-architecting.

---

## 3. Complete Polyglot Monorepo Structure

```
rey/
│
├── Cargo.toml                   # Virtual workspace manifest (Rust)
├── Cargo.lock
│
├── pnpm-workspace.yaml          # pnpm workspace config (JS/TS)
├── package.json                 # Root orchestration scripts
├── turbo.json                   # Turborepo task graph (optional, for scale)
│
├── crates/                      # Rust workspace members
│   ├── types/
│   ├── common/
│   ├── crypto/
│   ├── image/
│   ├── metadata/
│   ├── thumbnail/
│   ├── local-db/
│   ├── sync/
│   ├── zoo-client/
│   ├── zoo/                     # Monolithic server (internal modules)
│   ├── client-lib/              # Tauri backend commands
│   └── zoo-wasm/                # WASM bindings
│
├── apps/
│   ├── desktop/                 # Tauri 2.0 app (frontend + Tauri config)
│   │   ├── src/                 # Frontend (React/Svelte/Solid)
│   │   ├── src-tauri/           # Tauri Rust side (thin, imports client-lib)
│   │   ├── src-ios/             # Swift native plugin code (Tauri plugin system)
│   │   ├── src-android/         # Kotlin native plugin code
│   │   ├── package.json
│   │   └── tauri.conf.json
│   │
│   └── web/                     # Next.js web app (Vercel) [optional]
│       ├── src/
│       │   ├── app/             # App Router pages
│       │   └── lib/             # Generated API client from utoipa spec
│       ├── next.config.ts
│       └── package.json
│
├── packages/                    # Shared JS/TS packages
│   ├── ui/                      # Shared UI components (React, shadcn-based)
│   ├── api-client/              # Generated TypeScript client from OpenAPI spec
│   ├── types/                   # Shared TypeScript types
│   ├── tsconfig/                # Shared TypeScript configs
│   └── eslint-config/           # Shared ESLint configs
│
├── docs/                        # Architecture, protocol specs, runbooks
│
├── scripts/                     # Build/release automation scripts
│
├── Makefile                     # Top-level orchestration (lingua franca)
│
├── .github/
│   └── workflows/               # CI: cargo test --workspace, pnpm turbo build
│
└── CLAUDE.md                    # AI coding conventions (if using AI tooling)
```

### 3.1 Dual-Workspace Coexistence

The monorepo hosts two independent workspace systems that share the same root:

| System | Config file | Manages | Packages in |
|---|---|---|---|
| Cargo | `Cargo.toml` | Rust crates | `crates/*` |
| pnpm | `pnpm-workspace.yaml` | JS/TS packages | `apps/*`, `packages/*` |

This is the same pattern used by Turborepo itself, Tauri, and Cap — all of which are Cargo + pnpm monorepos. The two systems are unaware of each other but are orchestrated at the root level via `turbo.json` or `Makefile`.

A root `Makefile` serves as the lingua franca:

```
dev:
    make -j2 dev-desktop dev-server

dev-desktop:
    cd apps/desktop && pnpm tauri dev

dev-server:
    cd crates/zoo && cargo watch -x run

gen-api-client:
    cd crates/zoo && cargo run --bin gen-openapi
    cd packages/api-client && pnpm generate

build:
    cd crates && cargo build --release
    pnpm turbo build

test:
    cargo test --workspace
    pnpm turbo test
```

### 3.2 Type Contract Between Rust and TypeScript

The Rust server crate (`zoo`) uses `utoipa` to generate an OpenAPI specification directly from handler types. The TypeScript client (`packages/api-client`) is auto-generated from that spec using `openapi-typescript` or `orval`. This means:

- Rust types are the source of truth
- TypeScript types are generated, never hand-written
- Adding a field in Rust and forgetting the frontend causes a TypeScript compilation error
- The contract is always accurate

For Tauri's IPC (desktop/mobile apps), `tauri_specta` generates fully typed TypeScript bindings from Rust command definitions. Same guarantee: Rust types flow downstream.

### 3.3 Where Each Encryption Path Executes

| Platform | Encryption Runs In | Crypto Source |
|---|---|---|---|
| Desktop (Tauri) | `crypto` Rust crate | Same compiled binary (native) |
| Web browser | `crypto` Rust crate | WASM (compiled from same crate) |

The server never receives plaintext or keys in any scenario.

---

## 4. Recommended Starting Point

If building incrementally, start with:

```
crates/
├── types/              # Do this first — defines the entire data contract
├── crypto/             # Do this second — core encryption primitives
├── image/              # Do this third — payload embedding
├── client-lib/         # Do this fourth — Tauri command layer
└── zoo/                # Do alongside — coordination server

apps/
├── desktop/            # Start here — Tauri app
└── web/                # Add later if needed — Next.js web app
```

This minimizes upfront investment while establishing the correct architectural boundaries. The crate boundaries are the hard part. The workspace is just plumbing.

---

## 5. Decision Matrix

| Concern | Tauri 2.0 | Vercel + Next.js | Pure Native (Swift+KT+Web) |
|---|---|---|---|
| Desktop platforms | All 3 (macOS/Win/Linux) | Web only (PWA) | Per-platform only |
| Mobile platforms | iOS + Android | Web only (PWA) | iOS or Android only |
| Codebase count | 1 frontend + 1 Rust | 1 frontend + 1 Rust server | 3 independent codebases |
| Binary size | ~600KB – 5MB | N/A (server-rendered) | Platform-native |
| Camera access | Yes (plugin) | Web limited | Full native |
| Keychain/biometrics | Yes (plugin) | No | Full native |
| Offline encryption | Yes (Rust) | Limited (Service Worker) | Yes (native) |
| Crypto audit surface | 1 Rust crate | Web Crypto + WASM | 3 implementations |
| App Store distributable | Yes | No (web only) | Yes |
| Team complexity | Full-stack + Rust | Full-stack JS/TS | Swift + KT + Rust + JS |
| Startup time | ~2 years mature (v2.11) | Very mature | Varies by platform |
| Bundle Node.js? | No | Yes (on server) | No |
| Swift/Kotlin needed? | Only for platform plugins | No | Entire UI + logic |
| Web version from same repo | Optional (Next.js in `apps/web`) | Primary target | Separate repo |

---

## 6. Key Architectural Invariants

1. **Plaintext never crosses the network boundary.** Encryption and decryption execute in `crypto` (Rust native or WASM). The server only handles ciphertext.

2. **Rust crates are the source of truth for all data types.** TypeScript types are generated downstream via OpenAPI spec or `tauri_specta`.

3. **Dependency flow is strictly one-way.** `crypto` and `image` know nothing about the server or the Tauri runtime. They are pure libraries with zero framework coupling.

4. **The Tauri app is a thin shell.** `apps/desktop/src-tauri/` contains no business logic. It imports `client-lib` (which imports `sync`, `local-db`, and `thumbnail`) and registers its commands. The frontend is equally thin — it calls `invoke()` and renders.

5. **Feature flags gate platform-specific code.** The `client-lib` crate has `#[cfg(target_os = "ios")]` and `#[cfg(target_os = "android")]` only for platform plugin bridges. The crypto path is identical on all platforms.

6. **Shared UI components live in `packages/ui`.** If a web app is added later, it consumes the same components as the Tauri frontend. TailwindCSS + shadcn is the convention established by multiple production Tauri monorepos.

7. **The Makefile or turbo.json orchestrates everything.** One `make dev` starts both the Tauri dev server and the Rust server. One `make build` produces the final artifacts. The orchestration layer is language-agnostic.
