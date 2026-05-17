#!/usr/bin/env bash
# build.sh — produce the release viewer bundle.
#
# Installs deps (frozen lockfile), runs the vite build, then zips
# `dist/` into `psychological-operations-viewer.zip` at the repo root.
# That zip filename matches `viewer_zip` in `objectiveai.json` and is
# what the GitHub release workflow uploads alongside the platform
# binaries.
#
# Usage:
#   bash psychological-operations-viewer/build.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

# `--frozen-lockfile` so CI fails fast if pnpm-lock.yaml is out of
# date relative to package.json (caught locally during `pnpm add`).
pnpm install --frozen-lockfile
pnpm build
node scripts/zip.mjs
