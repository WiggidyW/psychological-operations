#!/usr/bin/env python3
"""Publish one scrape per YC W22 CEO from yc-w22-ceos.json."""

import json
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlparse

REPO = Path(__file__).parent
INPUT = REPO / "yc-w22-ceos.json"
CLI = REPO / "psychological-operations-cli/target/debug/psychological-operations.exe"

def slugify(name: str) -> str:
    s = name.lower()
    s = re.sub(r"\([^)]*\)", lambda m: " " + m.group(0)[1:-1] + " ", s)
    s = re.sub(r"[^a-z0-9]+", "-", s)
    return s.strip("-")

def handle_from_url(url: str) -> str:
    parsed = urlparse(url)
    parts = [p for p in parsed.path.split("/") if p]
    if not parts:
        raise ValueError(f"no handle in {url}")
    return parts[-1]

def main() -> int:
    entries = json.loads(INPUT.read_text())
    failed = []
    for e in entries:
        try:
            handle = handle_from_url(e["x"])
        except Exception as exc:
            print(f"!! {e['name']}: bad url {e['x']}: {exc}", file=sys.stderr)
            failed.append(e["name"])
            continue
        slug = slugify(e["name"])
        name = f"yc-unsettling-{slug}"
        scrape = {
            "agent": {
                "remote": "filesystem",
                "owner": "objectiveai",
                "repository": "opus-claude-agent-sdk",
            },
            "filters": [{"query": f"from:{handle}"}],
            "tags": ["yc-unsettling", slug],
            "count": 30,
        }
        body = json.dumps(scrape)
        msg = f"publish {name} (from:{handle})"
        result = subprocess.run(
            [str(CLI), "scrapes", "publish",
             "--name", name,
             "--scrape-inline", body,
             "--message", msg],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            print(f"!! {name}: publish failed: {result.stderr.strip()}", file=sys.stderr)
            failed.append(name)
            continue
        print(f"ok  {name}: {result.stdout.strip()}")

    if failed:
        print(f"\nfailed: {len(failed)}/{len(entries)}: {failed}", file=sys.stderr)
        return 1
    print(f"\nall {len(entries)} scrapes published.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
