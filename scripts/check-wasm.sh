#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
: "${WASM_CONTRACT_DIR:?Set WASM_CONTRACT_DIR to the authoritative contracts directory}"
output=${1:?Usage: bash scripts/check-wasm.sh ARTIFACT_DIRECTORY}
python3 scripts/wasm-receipt.py check "$output" "$WASM_CONTRACT_DIR"
cargo build --package logifos-facade --example wire --locked -j 2
WASM_NATIVE_CONSUMER="${CARGO_TARGET_DIR:?}/debug/examples/wire" WASM_ARTIFACT_DIR="$output" node --test crates/logifos-wasm/tests/transport.mjs
