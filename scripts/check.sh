#!/usr/bin/env bash
# Every check CI runs, in one command. Run before finishing a change.
#   scripts/check.sh        check only
#   scripts/check.sh --fix  apply formatter and safe lint fixes first
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ "${1:-}" == "--fix" ]]; then
  cargo fmt --all
  (cd web && npx biome check --write .)
fi

echo "==> rust: fmt";      cargo fmt --all --check
echo "==> rust: clippy";   cargo clippy --workspace --all-targets --locked -- -D warnings
echo "==> rust: clippy (wasm32)"
cargo clippy -p sim-wasm --target wasm32-unknown-unknown --locked -- -D warnings
echo "==> rust: test";     cargo test -p sim --locked
echo "==> web: wasm";      (cd web && npm run --silent wasm)
echo "==> web: biome";     (cd web && npx biome ci .)
echo "==> web: typecheck"; (cd web && npm run --silent typecheck)
echo "All checks passed."
