.PHONY: dev dev-server dev-web build serve tunnel share test check

# Development, everything in one terminal: the server (:3000) and Vite (:5173) together.
# Open http://localhost:5173. Ctrl-C stops both. The server logs and Vite's output share the terminal.
dev:
	@[ -d web/node_modules ] || npm ci --prefix web
	cargo build -p chessy-server
	@trap 'kill $$SERVER 2>/dev/null' EXIT INT TERM; \
	target/debug/chessy-server & SERVER=$$!; \
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

# Public Cloudflare quick tunnel to the server on :3000 (the URL is printed in the output)
tunnel:
	cloudflared tunnel --url http://127.0.0.1:3000

# Build the client, then run the server and the tunnel together; Ctrl-C stops both
share: build
	cargo build --release -p chessy-server
	@trap 'kill $$SERVER 2>/dev/null' EXIT INT TERM; \
	target/release/chessy-server & SERVER=$$!; \
	cloudflared tunnel --url http://127.0.0.1:3000

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
