#!/usr/bin/env python3
"""Publish one psyop per YC W22 CEO that scores their tweets via the
unsettlingness-ranker function over the yc-unsettling-swarm."""

import json
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlparse

REPO = Path(__file__).parent
INPUT = REPO / "yc-w22-ceos.json"
CLI = REPO / "psychological-operations-cli/target/debug/psychological-operations.exe"

# Three-agent swarm we already published. Inlined here as the Auto-profile
# body so each psyop carries its own copy (Profile.Auto = swarm inline).
AUTO_PROFILE = {
    "agents": [
        {
            "upstream": "openrouter",
            "model": "openai/gpt-4o-mini",
            "output_mode": "json_schema",
            "top_logprobs": 20,
            "count": 1,
        },
        {
            "upstream": "claude_agent_sdk",
            "model": "haiku",
            "output_mode": "instruction",
            "count": 1,
        },
        {
            "upstream": "claude_agent_sdk",
            "model": "sonnet",
            "output_mode": "instruction",
            "count": 1,
        },
    ],
}

FUNCTION_REF = {
    "remote": "filesystem",
    "owner": "ObjectiveAI",
    "repository": "unsettlingness-ranker",
}

STRATEGY = {"type": "swiss_system", "pool": 10, "rounds": 2}

def slugify(name: str) -> str:
    s = name.lower()
    s = re.sub(r"\([^)]*\)", lambda m: " " + m.group(0)[1:-1] + " ", s)
    s = re.sub(r"[^a-z0-9]+", "-", s)
    return s.strip("-")

def main() -> int:
    entries = json.loads(INPUT.read_text())
    failed = []
    for e in entries:
        slug = slugify(e["name"])
        name = f"yc-unsettling-{slug}"
        psyop = {
            "sources": [{"tag": slug, "count": 30}],
            "tags": ["yc-unsettling-scored", slug],
            "function": FUNCTION_REF,
            "profile": AUTO_PROFILE,
            "strategy": STRATEGY,
            "count": 10,
        }
        body = json.dumps(psyop)
        msg = f"publish {name} (score yc-unsettling tweets via unsettlingness-ranker swarm)"
        result = subprocess.run(
            [str(CLI), "psyops", "publish",
             "--name", name,
             "--psyop-inline", body,
             "--message", msg],
            capture_output=True, text=True,
        )
        if result.returncode != 0:
            print(f"!! {name}: {result.stderr.strip()}", file=sys.stderr)
            failed.append(name)
            continue
        print(f"ok  {name}: {result.stdout.strip()}")

    if failed:
        print(f"\nfailed: {len(failed)}/{len(entries)}: {failed}", file=sys.stderr)
        return 1
    print(f"\nall {len(entries)} psyops published.")
    return 0

if __name__ == "__main__":
    sys.exit(main())
