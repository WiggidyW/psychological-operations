#!/usr/bin/env bash
# Builds the playwright wrapper as a standalone binary using pkg.
#
# Usage:
#   bash psychological-operations-playwright/build.sh [--release]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

PROFILE="debug"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    *) shift ;;
  esac
done

# Check if rebuild is needed
if source "$SCRIPT_DIR/fingerprint.sh" $( [ "$PROFILE" = "release" ] && echo "--release" || true ); then
  echo "Fingerprint changed — rebuilding..."
else
  echo "psychological-operations-playwright: UP TO DATE"
  exit 0
fi

# Detect platform for pkg target
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)   PKG_TARGET="node22-linux-x64";   TARGET="x86_64-unknown-linux-gnu";   EXT="" ;;
  Linux-aarch64)  PKG_TARGET="node22-linux-arm64";  TARGET="aarch64-unknown-linux-gnu";  EXT="" ;;
  Darwin-x86_64)  PKG_TARGET="node22-macos-x64";    TARGET="x86_64-apple-darwin";        EXT="" ;;
  Darwin-arm64)   PKG_TARGET="node22-macos-arm64";   TARGET="aarch64-apple-darwin";       EXT="" ;;
  MINGW*|MSYS*|CYGWIN*) PKG_TARGET="node22-win-x64"; TARGET="x86_64-pc-windows-msvc";   EXT=".exe" ;;
  *) echo "Unsupported platform" >&2; exit 1 ;;
esac

EMBED_DIR="$SCRIPT_DIR/embed/$TARGET/$PROFILE"
BINARY="$EMBED_DIR/psychological-operations-playwright${EXT}"

# Install dependencies
cd "$SCRIPT_DIR"
npm install --quiet 2>/dev/null

# Compile TypeScript
npx tsc

# Build with pkg
mkdir -p "$EMBED_DIR"
npx pkg dist/index.js \
  --target "$PKG_TARGET" \
  --output "$BINARY" \
  --no-bytecode \
  --public \
  --public-packages=* \
  --fallback-to-source

echo "psychological-operations-playwright: SUCCESS ($BINARY)"
