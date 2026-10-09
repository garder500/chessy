#!/bin/bash
# SessionStart hook: prepares Claude Code cloud sessions so tests and linters run.
# Rust workspace (crates/*) + web client (web/, npm).
set -euo pipefail

# Only run in remote (cloud) sessions.
if [ "${CLAUDE_CODE_REMOTE:-}" != "true" ]; then
  exit 0
fi

cd "${CLAUDE_PROJECT_DIR:-$(cd "$(dirname "$0")/../.." && pwd)}"

# Rust: make sure fmt/clippy are present, then fetch and pre-build dependencies.
if command -v rustup >/dev/null 2>&1; then
  rustup component add rustfmt clippy
fi
cargo fetch --locked
cargo test --workspace --no-run

# Web client: npm install (not ci) so the container cache is reused.
npm install --prefix web --no-audit --no-fund
