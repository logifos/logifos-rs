#!/usr/bin/env bash
# Builds the incumbent Lasem renderer for the canon comparison from the
# lasem-ruby checkout: the vendored Lasem C library (pinned by the gem's
# submodule) plus the gem's extension. Requires meson, ninja, a modern
# bison (Xcode's cannot parse the itex2MML grammar flags) and the gem's
# bundle.
set -euo pipefail
cd "$(dirname "$0")/.."
gem_dir=${LASEM_RUBY_DIR:?set LASEM_RUBY_DIR to the lasem-ruby checkout}
revision=$(node -e 'console.log(require("./corpus/canon/lasem.json").revision)')
cd "$gem_dir"
actual=$(git -C vendor/lasem/source rev-parse HEAD)
if [ "$actual" != "$revision" ]; then
  echo "vendored Lasem is at $actual, pinned to $revision (git submodule update --init vendor/lasem/source)"
  exit 2
fi
if [ ! -f vendor/lasem/install/lib/pkgconfig/lasem-0.6.pc ]; then
  PATH="/opt/homebrew/opt/bison/bin:$PATH" bundle exec rake lasem:build
fi
if [ ! -f lib/lasem/lasem.bundle ]; then
  PKG_CONFIG_PATH="$PWD/vendor/lasem/install/lib/pkgconfig" bundle exec rake compile
fi
echo "lasem built against $actual"
