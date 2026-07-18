.PHONY: dev build test lint clean help

.DEFAULT_GOAL := help

help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | \
	awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-20s\033[0m %s\n", $$1, $$2}'

dev:
	cargo run &

build:
	cargo build --release

build-frontend:
	cd frontend && npm install && npm run build

test:
	cargo test

lint:
	cargo clippy -- -D warnings
	cargo fmt -- --check

fmt:
	cargo fmt

clean:
	cargo clean
	rm -rf frontend/dist frontend/node_modules

check: test lint

tauri-dev:
	cargo build -p openlocalrouter

tauri-build:
	cargo build -p openlocalrouter --release
