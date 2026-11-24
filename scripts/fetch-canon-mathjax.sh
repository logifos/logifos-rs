#!/usr/bin/env bash
# Installs the pinned MathJax version for the canon comparison into
# corpus/canon/mathjax-env. Re-runs replace the environment wholesale.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(node -e 'console.log(require("./corpus/canon/mathjax.json").version)')
dest=corpus/canon/mathjax-env
rm -rf "$dest"
mkdir -p "$dest"
cd "$dest"
npm init --yes >/dev/null
npm install --no-audit --no-fund --save-exact "mathjax@$version" >/dev/null
installed=$(node -e 'console.log(require("mathjax/package.json").version)')
[ "$installed" = "$version" ] || { echo "installed $installed, pinned $version"; exit 1; }
echo "mathjax $installed installed"
