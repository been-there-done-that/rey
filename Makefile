.PHONY: dev dev-desktop dev-server build test lint fmt gen-openapi gen-bindings wasm wasm-clean

dev: dev-desktop dev-server

dev-desktop:
	cd apps/desktop && pnpm tauri dev

dev-server:
	cd crates/zoo && cargo watch -x run

build:
	cargo build --release
	pnpm turbo build

test:
	cargo test --workspace --all-features
	pnpm turbo test

lint:
	cargo clippy --workspace -- -D warnings
	pnpm turbo lint

fmt:
	cargo fmt --all

gen-openapi:
	cargo run --bin gen-openapi --manifest-path crates/zoo/Cargo.toml > openapi.json
	cd packages/api-client && pnpm generate

gen-bindings:
	cd apps/desktop/src-tauri && cargo run --bin gen-bindings

# Build WASM crypto module for browser
# Outputs to apps/web/src/wasm/ which is imported by the Next.js frontend
# Also copies .wasm to public/wasm/ for Turbopack-compatible serving
wasm:
	wasm-pack build crates/zoo-wasm --target web --out-dir apps/web/src/wasm --no-default-features
	mv crates/zoo-wasm/apps/web/src/wasm apps/web/src/wasm 2>/dev/null || true
	mkdir -p apps/web/public/wasm
	cp apps/web/src/wasm/zoo_wasm_bg.wasm apps/web/public/wasm/

# Clean and rebuild WASM from scratch
wasm-clean:
	rm -rf apps/web/src/wasm apps/web/public/wasm
	wasm-pack build crates/zoo-wasm --target web --out-dir apps/web/src/wasm --no-default-features
	mv crates/zoo-wasm/apps/web/src/wasm apps/web/src/wasm 2>/dev/null || true
	mkdir -p apps/web/public/wasm
	cp apps/web/src/wasm/zoo_wasm_bg.wasm apps/web/public/wasm/
