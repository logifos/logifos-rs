#!/usr/bin/env bash
# Fetches every reference corpus at its pinned revision into
# corpus/reference/<id>. Re-runs are idempotent: an existing entry with
# the pinned revision recorded in .fetched passes through.
set -euo pipefail
cd "$(dirname "$0")/.."
node -e '
const { sources } = require("./corpus/reference/sources.json");
for (const s of sources) console.log(s.id + " " + s.repository + " " + s.subpath);
' | while read -r id repo subpath; do
  dest="corpus/reference/$id"
  if [ -d "$dest" ] && [ -f "$dest/.fetched" ]; then
    echo "skip $id (fetched)"
    continue
  fi
  tmp=$(mktemp -d)
  git clone --quiet --depth 1 "$repo" "$tmp/clone"
  git -C "$tmp/clone" checkout --quiet FETCH_HEAD
  mkdir -p "$dest"
  cp -R "$tmp/clone/$subpath/." "$dest/"
  git -C "$tmp/clone" rev-parse HEAD > "$dest/.fetched"
  rm -rf "$tmp"
  echo "fetched $id at $(cat "$dest/.fetched")"
done
