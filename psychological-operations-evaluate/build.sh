#!/usr/bin/env bash
# Sets up venv and installs the psychological-operations-evaluate package (editable).
# Output is captured to .logs/build/psychological-operations-evaluate.txt.
#
# Usage:
#   bash psychological-operations-evaluate/build.sh

set -euo pipefail

MODULE="psychological-operations-evaluate"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VENV_DIR="$SCRIPT_DIR/venv"
LOG_DIR="$REPO_ROOT/.logs/build"
LOG_FILE="$LOG_DIR/$MODULE.txt"

mkdir -p "$LOG_DIR"

run() {
  # ── venv setup ──────────────────────────────────────────────────────────────────

  if [ ! -d "$VENV_DIR" ]; then
    echo "Creating virtual environment..."
    python3 -m venv "$VENV_DIR"
  fi

  if [ -d "$VENV_DIR/Scripts" ]; then
    PYTHON="$VENV_DIR/Scripts/python.exe"
    PIP="$VENV_DIR/Scripts/pip.exe"
  else
    PYTHON="$VENV_DIR/bin/python"
    PIP="$VENV_DIR/bin/pip"
  fi

  # ── stage README + LICENSE (pyproject.toml references them; gitignored,
  # never committed in psychological-operations-evaluate/).
  cp "$REPO_ROOT/README.md" "$SCRIPT_DIR/README.md"
  cp "$REPO_ROOT/LICENSE"   "$SCRIPT_DIR/LICENSE"

  # ── install dev requirements (pytest, pytest-asyncio) ──────────────────────────
  if ! "$PYTHON" -c "import pytest" 2>/dev/null; then
    echo "Installing dev requirements..."
    "$PIP" install -r "$SCRIPT_DIR/requirements-dev.txt" --quiet
  fi

  # ── install runtime requirements from PyPI (objectiveai, cocoindex,
  # objectiveai-cocoindex). Pinned versions live in requirements.txt and
  # mirror pyproject.toml [project.dependencies].
  "$PIP" install -r "$SCRIPT_DIR/requirements.txt" --quiet

  # ── editable install of psychological-operations-evaluate itself.
  if ! "$PYTHON" -c "import psychological_operations_evaluate" 2>/dev/null; then
    echo "Editable-installing psychological-operations-evaluate..."
    "$PIP" install -e "$SCRIPT_DIR" --quiet
  fi
}

if run > "$LOG_FILE" 2>&1; then
  echo "$MODULE: SUCCESS"
else
  echo "$MODULE: ERROR"
  exit 1
fi
