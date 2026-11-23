#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$repo_root"
if [ "$(df -Pm "$repo_root" | awk 'NR==2 {print $4}')" -lt 600 ]; then
    echo "Insufficient disk space for native qualification" >&2
    exit 1
fi
case "$(uname -s)" in
    Linux) library_name=liblogifos_ffi.so ;;
    Darwin) library_name=liblogifos_ffi.dylib ;;
    *) echo "Native smoke requires a qualified Unix host" >&2; exit 1 ;;
esac
target_dir=${CARGO_TARGET_DIR:-"$repo_root/target"}
case "$target_dir" in
    /*) ;;
    *) target_dir="$repo_root/$target_dir" ;;
esac
if [ "$#" -eq 0 ]; then
    cargo build --package logifos-ffi --locked -j 2
    library="$target_dir/debug/$library_name"
    receipt="$target_dir/debug/logifos-ffi.receipt.json"
    python3 scripts/ffi-artifact.py write --library "$library" --receipt "$receipt"
elif [ "$#" -eq 2 ] && [ "$1" = "--prebuilt" ]; then
    receipt=$(cd -- "$(dirname -- "$2")" && pwd)/$(basename -- "$2")
    library="$(dirname -- "$receipt")/$library_name"
else
    echo "Usage: bash scripts/check-ffi.sh [--prebuilt /path/to/logifos-ffi.receipt.json]" >&2
    exit 2
fi
python3 scripts/ffi-artifact.py check --library "$library" --receipt "$receipt"
python3 scripts/ffi-artifact.py test --library "$library" --receipt "$receipt"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/logifos-ffi.XXXXXX")
trap 'rm -rf -- "$scratch"' EXIT
library_dir=$(dirname -- "$library")
"${CC:-cc}" -std=c11 -Wall -Wextra -Werror \
    -I crates/logifos-ffi/include crates/logifos-ffi/tests/smoke.c \
    -L "$library_dir" -Wl,-rpath,"$library_dir" -llogifos_ffi \
    -o "$scratch/smoke"
"$scratch/smoke"
if command -v "${CXX:-c++}" >/dev/null 2>&1; then
    "${CXX:-c++}" -std=c++11 -Wall -Wextra -Werror -x c++ \
        -I crates/logifos-ffi/include crates/logifos-ffi/tests/smoke.c \
        -L "$library_dir" -Wl,-rpath,"$library_dir" -llogifos_ffi \
        -o "$scratch/smoke-cxx"
    "$scratch/smoke-cxx"
fi
