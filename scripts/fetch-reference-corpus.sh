#!/usr/bin/env bash
# Fetches every reference corpus at its pinned revision into
# corpus/reference/<id>. Re-runs are idempotent: an entry whose recorded
# revision matches the pin passes through.
set -euo pipefail
cd "$(dirname "$0")/.."

# Materializes one pinned commit into a temp checkout whose sparse cone is
# limited to `subpath`, then copies that subtree under `dest` (or copies the
# single file `file` when given). Idempotent: a destination whose recorded
# revision matches the pin passes through.
fetch_pinned() {
  local repo="$1" revision="$2" subpath="$3" dest="$4" file="${5-}"
  if [ -f "$dest/.fetched" ] && [ "$(cat "$dest/.fetched")" = "$revision" ] \
      && { [ -n "$file" ] || [ -d "$dest" ]; }; then
    echo "skip ${dest##*/} (at $revision)"
    return
  fi
  rm -rf "$dest"
  local tmp
  tmp=$(mktemp -d)
  # A blob-filtered cone sparse checkout keeps the big repos (WebKit,
  # google/fonts) to the adopted subtree; the pin is an exact commit.
  git init --quiet "$tmp/clone"
  git -C "$tmp/clone" remote add origin "$repo"
  git -C "$tmp/clone" sparse-checkout init --cone
  git -C "$tmp/clone" sparse-checkout set "$subpath"
  git -C "$tmp/clone" fetch --quiet --depth 1 --filter=blob:none origin "$revision"
  git -C "$tmp/clone" checkout --quiet FETCH_HEAD
  mkdir -p "$dest"
  if [ -n "$file" ]; then
    cp "$tmp/clone/$file" "$dest/"
  else
    cp -R "$tmp/clone/$subpath/." "$dest/"
  fi
  git -C "$tmp/clone" rev-parse FETCH_HEAD > "$dest/.fetched"
  rm -rf "$tmp"
  echo "fetched ${dest##*/} at $(cat "$dest/.fetched")"
}

node -e '
const { sources } = require("./corpus/reference/sources.json");
for (const s of sources) console.log(s.id + " " + s.repository + " " + s.revision + " " + s.subpath);
' | while read -r id repo revision subpath; do
  fetch_pinned "$repo" "$revision" "$subpath" "corpus/reference/$id"
done

mkdir -p corpus/fonts
# fontist-sourced fonts (Microsoft faces) are verified in place: fetch them
# with `fontist download <formula>` and copy the file to
# corpus/fonts/<id>/<file>; the script checks the pinned hash.
node -e '
const { fonts } = require("./corpus/fonts.json");
for (const f of fonts) console.log((f.repository ? "git" : "fontist") + " " + f.id + " " + (f.repository || f.fontist_formula) + " " + (f.revision || f.sha256) + " " + f.path + " " + f.sha256);
' | while read -r kind id repo revision path sha256; do
  if [ "$kind" = "fontist" ]; then
    file="corpus/fonts/$id/$(basename "$path")"
    if [ -f "$file" ] && [ "$(shasum -a 256 "$file" | awk '{print $1}')" = "$sha256" ]; then
      echo "skip $id (hash matches)"
    else
      echo "fetch $id with: fontist download $repo; copy the face to $file (sha256 $sha256)" >&2
      exit 1
    fi
  else
    fetch_pinned "$repo" "$revision" "$(dirname "$path")" "corpus/fonts/$id" "$path"
  fi
done
