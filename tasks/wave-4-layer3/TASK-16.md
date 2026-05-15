# Task 16: Implement `crates/zoo-wasm` — Layer 3 WASM Bindings

## Wave
4 (Layer 3 — Platform Bindings)

## Dependencies
- Task 1 (Scaffold) must be complete
- Task 2 (types) must be complete
- Task 12 (zoo-client) must be complete

## Can Run In Parallel With
- Task 15 (client-lib) — no dependencies between them

## Design References
- design.md §11.2: Web (WASM)
- ZOO.md §13.3: WASM Target

## Requirements
23.1, 23.3, 23.4, 24.4, 25.4

## Objective
WASM bindings for zoo-client. `#[wasm_bindgen]` wrappers for web use.

## CRITICAL CONSTRAINT
- **NO `crypto`, `image`, `metadata`, `thumbnail` dependencies** — enforced at compile time (Req 24.4)

## Cargo.toml
```toml
[package]
name = "zoo-wasm"
version = "0.1.0"
edition.workspace = true
license.workspace = true

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
types = { workspace = true }
zoo-client = { workspace = true }
wasm-bindgen = { workspace = true }
wasm-bindgen-futures = { workspace = true }
serde-wasm-bindgen = { workspace = true }
serde_json = { workspace = true }
console_error_panic_hook = "0.1"
```

## Files to Create

### `src/lib.rs`
```rust
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub struct ZooHandle {
    client: zoo_client::ZooClient,
}

#[wasm_bindgen]
impl ZooHandle {
    #[wasm_bindgen(constructor)]
    pub async fn create(config: JsValue) -> Result<ZooHandle, JsError> {
        let config: ZooConfig = serde_wasm_bindgen::from_value(config)?;
        let client = zoo_client::ZooClient::new(config.base_url);
        Ok(ZooHandle { client })
    }

    pub async fn upload_file(&self, encrypted_bytes: &[u8], metadata: JsValue)
        -> Result<JsValue, JsError>
    {
        let metadata: UploadMetadata = serde_wasm_bindgen::from_value(metadata)?;
        let file_id = self.client.upload_file(encrypted_bytes, metadata).await?;
        Ok(serde_wasm_bindgen::to_value(&file_id)?)
    }

    pub async fn resume_upload(&self, upload_id: &str, encrypted_bytes: &[u8])
        -> Result<JsValue, JsError>
    {
        let upload_id = uuid::Uuid::parse_str(upload_id)?;
        let file_id = self.client.resume_upload(upload_id, encrypted_bytes).await?;
        Ok(serde_wasm_bindgen::to_value(&file_id)?)
    }

    pub async fn pending_uploads(&self) -> Result<JsValue, JsError> {
        let uploads = self.client.pending_uploads().await?;
        Ok(serde_wasm_bindgen::to_value(&uploads)?)
    }

    pub async fn cancel_upload(&self, upload_id: &str) -> Result<(), JsError> {
        let upload_id = uuid::Uuid::parse_str(upload_id)?;
        self.client.cancel_upload(upload_id).await?;
        Ok(())
    }

    pub fn set_session_token(&mut self, token: &str) {
        self.client.set_session_token(token.to_string());
    }

    pub fn close(&self) {
        // Cleanup if needed
    }
}
```

### `src/config.rs` (internal)
```rust
#[derive(serde::Deserialize)]
pub struct ZooConfig {
    pub base_url: String,
}
```

## Build Process
- Add to Makefile: `wasm-pack build crates/zoo-wasm --target web --out-dir apps/web/src/wasm`
- Add to CI workflow

## Tests (Task 16.4 — marked with *)
Unit tests for WASM bindings:
- `ZooHandle::create` initialises client
- `upload_file` delegates to zoo-client orchestrator
- `cancel_upload` calls DELETE endpoint

## Verification Steps
- [ ] `cargo check -p zoo-wasm` succeeds
- [ ] `wasm-pack build crates/zoo-wasm --target web` succeeds
- [ ] NO crypto/image/metadata/thumbnail in `cargo tree -p zoo-wasm`
- [ ] Generated `.wasm` and `.js` files in output directory

## Notes
- The `cdylib` crate-type is required for WASM output.
- `console_error_panic_hook` routes Rust panics to the browser console.
- The JS side manages the SSE `EventSource` natively (not through WASM).
- `serde-wasm-bindgen` converts between JS values and Rust types.
- Encryption on the web side is handled by a separate WASM build of the `crypto` crate.
