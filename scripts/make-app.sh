#!/bin/sh
# Build marg.app around a marg binary (concept §11, "macOS steps"):
#
#   scripts/make-app.sh target/dist/marg out/
#
# Writes <outdir>/marg.app with Info.plist (version from Cargo.toml) and
# marg.icns. On macOS the bundle is then signed ad hoc (no Apple
# Developer account; Gatekeeper still asks once, see the README).
set -eu
bin=$1
out=$2
root=$(cd "$(dirname "$0")/.." && pwd)
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
app="$out/marg.app"
rm -rf "$app"
mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
cp "$bin" "$app/Contents/MacOS/marg"
cp "$root/crates/marg/assets/marg.icns" "$app/Contents/Resources/marg.icns"
sed "s/@VERSION@/$version/g" "$root/crates/marg/assets/Info.plist" > "$app/Contents/Info.plist"
if command -v codesign > /dev/null; then
  codesign --force --sign - "$app"
fi
echo "$app ($version)"
