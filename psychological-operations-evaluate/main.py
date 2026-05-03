#!/usr/bin/env python3
"""psychological-operations-evaluate — PyInstaller-bundled runner stub."""

from __future__ import annotations

__version__ = "0.1.0"

import sys

# Imported solely to force PyInstaller to bundle these into the binary.
# Real entry point is TBD; for now the runner exits 0 immediately.
import cocoindex  # noqa: F401
import objectiveai  # noqa: F401
import objectiveai_cocoindex  # noqa: F401


def main() -> None:
    sys.exit(0)


if __name__ == "__main__":
    main()
