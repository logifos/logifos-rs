#!/usr/bin/env bash
# Builds the pinned JEuclid fork into corpus/canon/jeuclid-env for the canon
# comparison. The fork predates modern JDKs: its poms pin source 1.5/1.6 (the
# build raises them to 8) and its gmaven plugin needs java.base opened. The
# support module installs its own build-tools plugin first.
set -euo pipefail
cd "$(dirname "$0")/.."
revision=$(node -e 'console.log(require("./corpus/canon/jeuclid.json").revision)')
repository=$(node -e 'console.log(require("./corpus/canon/jeuclid.json").repository)')
dest=corpus/canon/jeuclid-env
if [ -f "$dest/.built" ] && [ "$(cat "$dest/.built")" = "$revision" ]; then
  echo "skip jeuclid-env (at $revision)"
  exit 0
fi
rm -rf "$dest"
mkdir -p "$dest"
tmp=$(mktemp -d)
git init --quiet "$tmp/clone"
git -C "$tmp/clone" remote add origin "$repository"
git -C "$tmp/clone" sparse-checkout init --no-cone
git -C "$tmp/clone" sparse-checkout set '/*'
git -C "$tmp/clone" fetch --quiet --depth 1 --filter=blob:none origin "$revision"
git -C "$tmp/clone" checkout --quiet FETCH_HEAD
find "$tmp/clone" -name pom.xml -maxdepth 3 -exec sed -i '' \
  -e 's|<source>1\.[56]</source>|<source>8</source>|' \
  -e 's|<target>1\.[56]</target>|<target>8</target>|' {} \;
export MAVEN_OPTS="--add-opens java.base/java.io=ALL-UNNAMED --add-opens java.base/java.lang=ALL-UNNAMED --add-opens java.base/java.util=ALL-UNNAMED"
(cd "$tmp/clone/support" && mvn -q install -DskipTests)
(cd "$tmp/clone/jeuclid-core" && mvn -q package -DskipTests \
  && mvn -q dependency:copy-dependencies -DoutputDirectory="$PWD/libs")
cp -R "$tmp/clone" "$dest/src"
find "$dest/src/jeuclid-core/target" -name 'jeuclid-core-*.jar' \
  ! -name '*-sources.jar' ! -name '*-javadoc.jar' \
  -exec mv {} "$dest/src/jeuclid-core.jar" \;
rm -rf "$dest/src/jeuclid-core/target"
mkdir -p "$dest/classes"
javac -cp "$dest/src/libs/*:$dest/src/jeuclid-core.jar" \
  -d "$dest/classes" scripts/canon/jeuclid/CanonRender.java
git -C "$tmp/clone" rev-parse FETCH_HEAD > "$dest/.built"
rm -rf "$tmp"
echo "jeuclid built at $(cat "$dest/.built")"
