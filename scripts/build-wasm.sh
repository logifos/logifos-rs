#!/usr/bin/env bash
set -euo pipefail
repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
: "${WASM_BINDGEN:?Set WASM_BINDGEN to the task-local wasm-bindgen executable}"
: "${WASM_CONTRACT_DIR:?Set WASM_CONTRACT_DIR to the authoritative contracts directory}"
: "${CARGO_TARGET_DIR:?Set CARGO_TARGET_DIR to a task-local target directory}"
output=${1:?Usage: bash scripts/build-wasm.sh NEW_OUTPUT_DIRECTORY}
[[ "$(rustup run 1.95.0 rustc --version)" == 'rustc 1.95.0 '* ]] || { echo 'Rust 1.95.0 required' >&2; exit 1; }
[[ "$("$WASM_BINDGEN" --version)" == 'wasm-bindgen 0.2.105' ]] || { echo 'wasm-bindgen 0.2.105 required' >&2; exit 1; }
[[ ! -e "$output" ]] || { echo 'Output directory must be new' >&2; exit 1; }
[[ $(df -Pm . | awk 'NR==2 {print $4}') -ge 600 ]] || { echo 'Insufficient disk space' >&2; exit 1; }
# A fixed linear-memory ceiling bounds growth. Traps require a fresh instance.
unset CARGO_ENCODED_RUSTFLAGS
export RUSTFLAGS="-C link-arg=--max-memory=67108864"
rustup run 1.95.0 cargo build --release --package logifos-wasm --target wasm32-unknown-unknown --locked -j 2
mkdir -p -- "$output"
output=$(cd -- "$output" && pwd)
for target in nodejs web; do
    "$WASM_BINDGEN" "$CARGO_TARGET_DIR/wasm32-unknown-unknown/release/logifos_wasm.wasm" \
        --target "$target" --out-dir "$output/$target" --out-name logifos
 done
python3 scripts/wasm-receipt.py create "$output" "$WASM_CONTRACT_DIR"
python3 scripts/wasm-receipt.py check "$output" "$WASM_CONTRACT_DIR"
