#!/usr/bin/env bash
# Computes a SHA256 fingerprint of all source inputs.
# Returns exit code 0 if fingerprint changed (rebuild needed), 1 if unchanged.
#
# Usage:
#   source fingerprint.sh [--release]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

PROFILE="debug"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --release) PROFILE="release"; shift ;;
    *) shift ;;
  esac
done

# Detect target triple
case "$(uname -s)-$(uname -m)" in
  Linux-x86_64)   TARGET="x86_64-unknown-linux-gnu" ;;
  Linux-aarch64)  TARGET="aarch64-unknown-linux-gnu" ;;
  Darwin-x86_64)  TARGET="x86_64-apple-darwin" ;;
  Darwin-arm64)   TARGET="aarch64-apple-darwin" ;;
  MINGW*|MSYS*|CYGWIN*) TARGET="x86_64-pc-windows-msvc" ;;
  *) TARGET="unknown" ;;
esac

EMBED_DIR="$SCRIPT_DIR/embed/$TARGET/$PROFILE"
FINGERPRINT_FILE="$EMBED_DIR/.fingerprint"

# Compute fingerprint from source inputs
HASH_INPUT="$PROFILE"
for f in "$SCRIPT_DIR"/src/*.ts "$SCRIPT_DIR"/package.json "$SCRIPT_DIR"/tsconfig.json; do
  if [ -f "$f" ]; then
    HASH_INPUT="$HASH_INPUT$(sha256sum "$f" | cut -d' ' -f1)"
  fi
done
FINGERPRINT=$(echo -n "$HASH_INPUT" | sha256sum | cut -d' ' -f1)

# Compare with stored fingerprint
if [ -f "$FINGERPRINT_FILE" ] && [ "$(cat "$FINGERPRINT_FILE")" = "$FINGERPRINT" ]; then
  return 1 2>/dev/null || exit 1
fi

# Store new fingerprint
mkdir -p "$EMBED_DIR"
echo -n "$FINGERPRINT" > "$FINGERPRINT_FILE"
return 0 2>/dev/null || exit 0
