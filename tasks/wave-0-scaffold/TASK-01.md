# Task 1: Scaffold the Cargo Workspace and Shared Tooling

## Wave
0 (Foundation — must complete before all other tasks)

## Dependencies
None. This is the first task.

## Can Run In Parallel With
Nothing. All other tasks depend on this.

## Design References
- design.md §2.1: Cargo Workspace Layout
- design.md §2.4: Feature Flags
- design.md §2.5: Build Order & Compilation Parallelism
- design.md §11.3: Monorepo Structure
- ARCHITECTURE.md §3.1: Dual-Workspace Coexistence

## Requirements
25.1, 25.7

## Objective
Create the root Cargo.toml virtual manifest, workspace dependency pins, feature flag configuration, JS monorepo tooling, Makefile, CI workflow, and empty crate directory stubs so that `cargo check --workspace` resolves without errors.

## Files to Create

### 1. Root `Cargo.toml` (Virtual Workspace Manifest)
Create `Cargo.toml` at the repo root with:
- `[workspace]` section listing ALL members:
  - `crates/types`, `crates/common`, `crates/crypto`, `crates/image`, `crates/metadata`, `crates/thumbnail`, `crates/local-db`, `crates/sync`, `crates/zoo-client`, `crates/zoo`, `crates/client-lib`, `crates/zoo-wasm`
  - `apps/desktop/src-tauri`, `apps/web` (if applicable)
- `[workspace.package]` with: `edition = "2021"`, `license = "MIT"`, `repository = "..."` 
- `[workspace.dependencies]` pinning ALL shared crates with exact versions:
  - `serde = { version = "1", features = ["derive"] }`
  - `serde_json = "1"`
  - `tokio = { version = "1", features = ["full"] }`
  - `axum = { version = "0.7", features = ["macros"] }`
  - `sqlx = { version = "0.7", features = ["runtime-tokio", "postgres", "migrate", "macros"] }`
  - `reqwest = { version = "0.11", default-features = false, features = ["json", "stream"] }`
  - `argon2 = "0.5"`
  - `blake2b_simd = "1"`
  - `chacha20poly1305 = "0.10"`
  - `xsalsa20poly1305 = "0.9"`
  - `x25519-dalek = { version = "2", features = ["static_secrets"] }`
  - `zeroize = "1"`
  - `uuid = { version = "1", features = ["v4", "serde"] }`
  - `bcrypt = "0.15"`
  - `lru = "0.12"`
  - `dashmap = "5"`
  - `rusqlite = { version = "0.30", features = ["sqlcipher", "bundled"] }`
  - `keyring = "2"`
  - `wasm-bindgen = "0.2"`
  - `wasm-bindgen-futures = "0.4"`
  - `serde-wasm-bindgen = "0.6"`
  - `thiserror = "1"`
  - `tracing = "0.1"`
  - `tracing-subscriber = { version = "0.3", features = ["env-filter"] }`
  - `tower = "0.4"`
  - `tower-http = { version = "0.5", features = ["trace"] }`
  - `aws-sdk-s3 = "1"`
  - `aws-config = "1"`
  - `utoipa = { version = "4", features = ["axum_extras"] }`
  - `tauri = { version = "2", features = [] }`
  - `tauri-specta = { version = "2", features = ["derive", "typescript"] }`
  - `specta = "2"`
  - `rand_core = "0.6"`
  - `proptest = "1"`
  - `wiremock = "0.5"`
  - `tempfile = "3"`
  - `base64 = "0.21"`
  - `hex = "0.4"`
  - `image = "0.25"`
  - `kamadak-exif = "0.5"`
  - `secrecy = "0.8"`
- `[profile.dev.package.crypto]` with `opt-level = 3` (keep encryption fast in debug builds)
- `[profile.dev]` with `debug = true`
- `[profile.release]` with `lto = "thin"`, `opt-level = 3`

### 2. `pnpm-workspace.yaml`
```yaml
packages:
  - 'apps/*'
  - 'packages/*'
```

### 3. Root `package.json`
```json
{
  "name": "rey",
  "private": true,
  "scripts": {
    "dev": "turbo run dev",
    "build": "turbo run build",
    "test": "turbo run test",
    "lint": "turbo run lint",
    "typecheck": "turbo run typecheck"
  },
  "devDependencies": {
    "turbo": "^2.0"
  },
  "packageManager": "pnpm@9.0.0"
}
```

### 4. `turbo.json`
```json
{
  "$schema": "https://turbo.build/schema.json",
  "tasks": {
    "build": {
      "dependsOn": ["^build"],
      "outputs": ["dist/**", ".next/**"]
    },
    "dev": {
      "cache": false,
      "persistent": true
    },
    "test": {
      "dependsOn": ["build"]
    },
    "lint": {},
    "typecheck": {}
  }
}
```

### 5. `Makefile`
Create with these targets:
- `dev`: runs `dev-desktop` and `dev-server` in parallel (`make -j2`)
- `dev-desktop`: `cd apps/desktop && pnpm tauri dev`
- `dev-server`: `cd crates/zoo && cargo watch -x run`
- `build`: `cd crates && cargo build --release` then `pnpm turbo build`
- `test`: `cargo test --workspace --all-features` then `pnpm turbo test`
- `lint`: `cargo clippy --workspace -- -D warnings` then `pnpm turbo lint`
- `fmt`: `cargo fmt --all`
- `gen-openapi`: `cargo run --bin gen-openapi --manifest-path crates/zoo/Cargo.toml > openapi.json` then `cd packages/api-client && pnpm generate`
- `gen-bindings`: `cd apps/desktop/src-tauri && cargo run --bin gen-bindings`
- `wasm-pack`: `wasm-pack build crates/zoo-wasm --target web --out-dir ../../apps/web/src/wasm`

### 6. `.github/workflows/ci.yml`
Create a GitHub Actions workflow with:
- Job `rust`: steps for `cargo test --workspace --all-features`, `cargo clippy --workspace -- -D warnings`, `cargo fmt --check`
- Job `js`: steps for `pnpm install`, `pnpm turbo test`, `pnpm turbo lint`, `pnpm turbo typecheck`
- Use `ubuntu-latest` runner
- Set up Rust with `actions-rust-lang/setup-rust-toolchain@v1`
- Set up pnpm with `pnpm/action-setup@v4`

### 7. Empty Crate Directory Stubs
Create `crates/` with empty `src/lib.rs` for each:
- `crates/types/src/lib.rs` — just `// types crate`
- `crates/common/src/lib.rs` — just `// common crate`
- `crates/crypto/src/lib.rs` — just `// crypto crate`
- `crates/image/src/lib.rs` — just `// image crate`
- `crates/metadata/src/lib.rs` — just `// metadata crate`
- `crates/thumbnail/src/lib.rs` — just `// thumbnail crate`
- `crates/local-db/src/lib.rs` — just `// local-db crate`
- `crates/sync/src/lib.rs` — just `// sync crate`
- `crates/zoo-client/src/lib.rs` — just `// zoo-client crate`
- `crates/zoo/src/lib.rs` — just `// zoo crate`
- `crates/client-lib/src/lib.rs` — just `// client-lib crate`
- `crates/zoo-wasm/src/lib.rs` — just `// zoo-wasm crate`

Each crate needs a minimal `Cargo.toml`:
```toml
[package]
name = "<crate-name>"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[dependencies]
# Will be populated by subsequent tasks
```

### 8. App Directory Stubs
- `apps/desktop/src-tauri/Cargo.toml` — minimal with `tauri` and `client-lib` deps
- `apps/desktop/tauri.conf.json` — minimal config with identifier `"com.rey.app"`
- `apps/web/package.json` — minimal with Next.js dependency

## Implementation Steps
1. Create root `Cargo.toml` with workspace, dependencies, profiles
2. Create `pnpm-workspace.yaml`, `package.json`, `turbo.json`
3. Create `Makefile` with all targets
4. Create `.github/workflows/ci.yml`
5. Create all 12 crate directories with minimal `Cargo.toml` and `src/lib.rs`
6. Create app stubs
7. Run `cargo check --workspace` — must succeed
8. Run `cargo fmt --all`

## Verification Steps
- [ ] `cargo check --workspace` succeeds with no errors
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy --workspace -- -D warnings` passes (may have warnings for empty crates, that's OK)
- [ ] All 12 crates are listed in `cargo metadata --no-deps --format-version 1 | jq '.packages[].name'`
- [ ] `[workspace.dependencies]` contains all pinned versions
- [ ] `[profile.dev.package.crypto]` has `opt-level = 3`
- [ ] `Makefile` has all required targets
- [ ] `.github/workflows/ci.yml` has both `rust` and `js` jobs

## Notes
- This task creates the skeleton. All crates are empty stubs.
- Subsequent tasks will populate each crate's `Cargo.toml` dependencies and source code.
- The `crypto` crate's `opt-level = 3` in dev profile is critical — encryption is too slow at default debug optimization.
- Feature flags will be configured in each crate's `Cargo.toml` by subsequent tasks: `crypto` gets `std`/`no_std`, `zoo` gets `s3`/`local-fs`, `client-lib` gets `desktop`, `local-db` gets `sqlcipher`.
