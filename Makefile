.PHONY: setup dev check check-web check-rust test audit package

setup:
	npm ci

dev:
	npm run tauri dev

check: check-web check-rust

check-web:
	npm run check

check-rust:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings
	cargo test --workspace --all-features

test:
	npm test
	cargo test --workspace --all-features

audit:
	npx audit-ci@7.1.0 --config audit-ci.json
	cargo install cargo-audit --locked
	cargo audit

package:
	npm run tauri build

