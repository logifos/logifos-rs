#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
target_dir=${CARGO_TARGET_DIR:-"$repo_root/target"}
case "$target_dir" in
    /*) ;;
    *) target_dir="$repo_root/$target_dir" ;;
esac
scratch=$(mktemp -d "${TMPDIR:-/tmp}/logifos-ffi.XXXXXX")
trap 'rm -rf -- "$scratch"' EXIT

cargo build --package logifos-ffi --locked -j 2
"${CC:-cc}" -std=c11 -Wall -Wextra -Werror \
    -I crates/logifos-ffi/include crates/logifos-ffi/tests/smoke.c \
    -L "$target_dir/debug" -Wl,-rpath,"$target_dir/debug" -llogifos_ffi \
    -o "$scratch/smoke"
"$scratch/smoke"
