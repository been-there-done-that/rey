.PHONY: dev dev-desktop dev-server build test lint fmt gen-openapi gen-bindings wasm-pack

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

wasm-pack:
	wasm-pack build crates/zoo-wasm --target web --out-dir ../../apps/web/src/wasm
