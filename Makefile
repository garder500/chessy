.PHONY: dev dev-server dev-web build serve tunnel share test check

# Untracked per-machine override, e.g. CHESSY_ADDR = 127.0.0.1:3001
-include local.mk

CHESSY_ADDR ?= 127.0.0.1:3000
export CHESSY_ADDR

# Development, everything in one terminal: the server ($(CHESSY_ADDR), override: make dev CHESSY_ADDR=127.0.0.1:3001) and Vite (:5173) together.
# Open http://localhost:5173. Ctrl-C stops both. The server logs and Vite's output share the terminal.
dev:
	@[ -d web/node_modules ] || npm ci --prefix web
	cargo build -p chessy-server
	@trap 'kill $$SERVER 2>/dev/null' EXIT INT TERM; \
	target/debug/chessy-server & SERVER=$$!; \
	sleep 1; \
	kill -0 $$SERVER 2>/dev/null || { echo "chessy-server failed to start: is $(CHESSY_ADDR) already in use? Try: make dev CHESSY_ADDR=127.0.0.1:3001" >&2; exit 1; }; \
	npm run dev --prefix web

# Or run these two in separate terminals
dev-server:
	cargo run -p chessy-server

dev-web:
	npm run dev --prefix web

# Production-style: build the client, then let the server serve it on :3000
build:
	npm ci --prefix web && npm run build --prefix web

serve: build
	cargo run --release -p chessy-server

# Public Cloudflare quick tunnel to the server (the URL is printed in the output)
tunnel:
	cloudflared tunnel --url http://$(CHESSY_ADDR)

# Build the client, then run the server and the tunnel together; Ctrl-C stops both
share: build
	cargo build --release -p chessy-server
	@trap 'kill $$SERVER 2>/dev/null' EXIT INT TERM; \
	target/release/chessy-server & SERVER=$$!; \
	cloudflared tunnel --url http://$(CHESSY_ADDR)

test:
	cargo test --workspace
	npm test --prefix web

# Everything CI should run
check:
	cargo fmt --all --check
	cargo clippy --workspace --all-targets -- -D warnings
	cargo test --workspace
	npm run build --prefix web
	npm test --prefix web
