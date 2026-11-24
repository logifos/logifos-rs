#!/usr/bin/env bash
# Fetches every reference corpus at its pinned revision into
# corpus/reference/<id>. Re-runs are idempotent: an entry whose recorded
# revision matches the pin passes through.
set -euo pipefail
cd "$(dirname "$0")/.."
node -e '
const { sources } = require("./corpus/reference/sources.json");
for (const s of sources) console.log(s.id + " " + s.repository + " " + s.revision + " " + s.subpath);
' | while read -r id repo revision subpath; do
  dest="corpus/reference/$id"
  if [ -f "$dest/.fetched" ] && [ "$(cat "$dest/.fetched")" = "$revision" ]; then
    echo "skip $id (at $revision)"
    continue
  fi
  rm -rf "$dest"
  tmp=$(mktemp -d)
  # A blob-filtered cone sparse checkout of the adopted subpath keeps the
  # big repos (WebKit) to that subtree; the pin is an exact commit.
  git init --quiet "$tmp/clone"
  git -C "$tmp/clone" remote add origin "$repo"
  git -C "$tmp/clone" sparse-checkout init --cone
  git -C "$tmp/clone" sparse-checkout set "$subpath"
  git -C "$tmp/clone" fetch --quiet --depth 1 --filter=blob:none origin "$revision"
  git -C "$tmp/clone" checkout --quiet FETCH_HEAD
  mkdir -p "$dest"
  cp -R "$tmp/clone/$subpath/." "$dest/"
  git -C "$tmp/clone" rev-parse FETCH_HEAD > "$dest/.fetched"
  rm -rf "$tmp"
  echo "fetched $id at $(cat "$dest/.fetched")"
done
