#!/usr/bin/env bash
# Validates that the embedded binary exists and its fingerprint is current.
# Exit codes: 0 = valid, 1 = missing, 2 = stale fingerprint.
#
# Usage:
#   bash validate.sh [--release]

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

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

# Check binary exists
if [ "$(uname -s)" = "MINGW"* ] || [ "$(uname -s)" = "MSYS"* ] || [ "$(uname -s)" = "CYGWIN"* ]; then
  BINARY="$EMBED_DIR/psychological-operations-playwright.exe"
else
  BINARY="$EMBED_DIR/psychological-operations-playwright"
fi

if [ ! -f "$BINARY" ]; then
  echo "Binary not found: $BINARY" >&2
  echo "Run build.sh first." >&2
  exit 1
fi

# Check fingerprint
if ! source "$SCRIPT_DIR/fingerprint.sh" ${PROFILE:+--$( [ "$PROFILE" = "release" ] && echo "release" || true )}; then
  # fingerprint.sh returns 1 if unchanged (valid)
  exit 0
else
  # fingerprint.sh returns 0 if changed (stale)
  echo "Fingerprint mismatch — binary is stale. Run build.sh." >&2
  exit 2
fi
