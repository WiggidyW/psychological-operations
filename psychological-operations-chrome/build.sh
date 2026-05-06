#!/usr/bin/env bash
# Stages the embedded Chromium bundle + (eventually) the packed
# extension into embed/<target>/<profile>/. Re-run is a no-op via
# fingerprint short-circuit.
#
# Usage:
#   bash psychological-operations-chrome/build.sh [--release] [--target <triple>]
#
# Pinned Chrome for Testing version is read from VERSION.

set -euo pipefail

MODULE="psychological-operations-chrome"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
LOG_DIR="$REPO_ROOT/.logs/build"
LOG_FILE="$LOG_DIR/$MODULE.txt"
EXT_DIR="$REPO_ROOT/psychological-operations-chrome-extension"

mkdir -p "$LOG_DIR"

run() {
  # ── fingerprint short-circuit ───────────────────────────────────────────
  if ! source "$SCRIPT_DIR/fingerprint.sh" "$@"; then
    return 0
  fi

  EMBED_DIR="$SCRIPT_DIR/embed/$TARGET/$PROFILE"
  mkdir -p "$EMBED_DIR"

  # ── download Chrome for Testing ─────────────────────────────────────────
  CHROME_ZIP="$EMBED_DIR/chrome-bundle.zip"
  CHROME_URL="https://storage.googleapis.com/chrome-for-testing-public/${CHROME_VERSION}/${CFT_PLATFORM}/chrome-${CFT_PLATFORM}.zip"
  echo "Downloading $CHROME_URL ..."
  curl -fLsS "$CHROME_URL" -o "$CHROME_ZIP"
  CHROME_BYTES=$(wc -c < "$CHROME_ZIP" | tr -d ' ')
  echo "  -> $CHROME_BYTES bytes"

  # ── stage extension dir (extension.crx packing is a follow-up commit) ──
  STAGE_DIR="$EMBED_DIR/extension"
  rm -rf "$STAGE_DIR"
  mkdir -p "$STAGE_DIR"
  cp -r "$EXT_DIR"/. "$STAGE_DIR"/

  # ── write the launch entry path so the Rust side knows what to exec ────
  printf '%s\n' "$CHROME_LAUNCH_REL" > "$EMBED_DIR/launch-entry.txt"

  # ── write a metadata file for diagnostics + provenance ─────────────────
  cat > "$EMBED_DIR/bundle.meta.json" <<EOF
{
  "chrome_version":   "$CHROME_VERSION",
  "cft_platform":     "$CFT_PLATFORM",
  "rust_target":      "$TARGET",
  "profile":          "$PROFILE",
  "chrome_url":       "$CHROME_URL",
  "chrome_bytes":     $CHROME_BYTES,
  "launch_entry_rel": "$CHROME_LAUNCH_REL"
}
EOF

  # ── stamp fingerprint AFTER successful build ───────────────────────────
  echo "$CURRENT_FP" > "$FINGERPRINT_FILE"
  echo "Build complete (fingerprint: ${CURRENT_FP:0:12}...)"
}

if run "$@" > "$LOG_FILE" 2>&1; then
  echo "$MODULE: SUCCESS"
else
  echo "$MODULE: ERROR (see $LOG_FILE)"
  exit 1
fi
