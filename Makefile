.PHONY: dev build test lint install

install:
	npm install

dev:
	npm run tauri dev

build:
	npm run tauri build

test:
	cargo test --workspace
	npm run test

lint:
	cargo fmt --all
	cargo clippy --workspace -- -D warnings
