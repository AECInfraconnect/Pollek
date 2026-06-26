#!/usr/bin/env python3
"""Keep README product promises tied to the implementation task list.

The README is treated as a user-facing contract. This script extracts the
feature, workflow, and dashboard promises from README.md and verifies that
docs/README_PROMISE_TASKS.md was refreshed after those promises changed.
"""

from __future__ import annotations

import argparse
import hashlib
import re
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
README = ROOT / "README.md"
TASKS = ROOT / "docs" / "README_PROMISE_TASKS.md"

HASH_RE = re.compile(r"<!-- readme-promises-sha256: ([a-f0-9]{64}) -->")
COUNT_RE = re.compile(r"<!-- readme-promises-count: ([0-9]+) -->")


def _finish(current: list[str], promises: list[str]) -> list[str]:
    if current:
        promises.append(" ".join(part.strip() for part in current if part.strip()))
    return []


def extract_promises(readme: str) -> list[str]:
    """Extract stable README claims that imply product behavior."""

    promises: list[str] = []
    current: list[str] = []
    in_code_block = False

    for raw_line in readme.splitlines():
        line = raw_line.rstrip()
        stripped = line.strip()

        if stripped.startswith("```"):
            in_code_block = not in_code_block
            current = _finish(current, promises)
            continue

        if in_code_block:
            continue

        starts_promise = (
            line.startswith("- **")
            or bool(re.match(r"^[0-9]+\. \*\*", line))
            or bool(re.match(r"^\| \*\*[^|]+\*\* \|", line))
        )

        if starts_promise:
            current = _finish(current, promises)
            current = [stripped]
            continue

        if current and (line.startswith("  ") or line.startswith("    ")):
            current.append(stripped)
            continue

        current = _finish(current, promises)

    _finish(current, promises)

    lead_claims = [
        "local-first AI Agent Governance Runtime",
        "enforces and observes AI-agent, MCP, API, and tool-call activity",
        "speaks one contract to both local and cloud",
        "Policy-First / PEP-Transparent Philosophy",
        "3-Step Quickstart",
        "Local mode",
        "Optional cross-OS demo profiles",
        "Enterprise Cloud mode",
        "Download & verify",
    ]
    for claim in lead_claims:
        if claim in readme:
            promises.append(f"README claim: {claim}")

    return sorted(dict.fromkeys(promises))


def promise_digest(promises: list[str]) -> str:
    payload = "\n".join(promises).encode("utf-8")
    return hashlib.sha256(payload).hexdigest()


def replace_marker(pattern: re.Pattern[str], text: str, replacement: str) -> str:
    if not pattern.search(text):
        raise SystemExit(f"Missing marker in {TASKS}: {pattern.pattern}")
    return pattern.sub(replacement, text, count=1)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--print", action="store_true", help="Print the current README promise fingerprint.")
    parser.add_argument("--write", action="store_true", help="Update hash/count markers in the task-list document.")
    args = parser.parse_args()

    readme_text = README.read_text(encoding="utf-8")
    promises = extract_promises(readme_text)
    digest = promise_digest(promises)

    if args.print:
        print(f"readme-promises-sha256={digest}")
        print(f"readme-promises-count={len(promises)}")
        return 0

    if not TASKS.exists():
        print(f"{TASKS} is missing. Create it before changing README promises.", file=sys.stderr)
        return 1

    tasks_text = TASKS.read_text(encoding="utf-8")
    hash_match = HASH_RE.search(tasks_text)
    count_match = COUNT_RE.search(tasks_text)

    if args.write:
        updated = replace_marker(HASH_RE, tasks_text, f"<!-- readme-promises-sha256: {digest} -->")
        updated = replace_marker(COUNT_RE, updated, f"<!-- readme-promises-count: {len(promises)} -->")
        TASKS.write_text(updated, encoding="utf-8", newline="\n")
        return 0

    if not hash_match or not count_match:
        print(f"{TASKS} must contain readme-promises hash and count markers.", file=sys.stderr)
        return 1

    expected_count = int(count_match.group(1))
    if hash_match.group(1) != digest or expected_count != len(promises):
        print("README promises changed without updating docs/README_PROMISE_TASKS.md.", file=sys.stderr)
        print(f"Expected hash/count in doc: {hash_match.group(1)} / {expected_count}", file=sys.stderr)
        print(f"Current README hash/count: {digest} / {len(promises)}", file=sys.stderr)
        print("Update the task list, then run: python scripts/check_readme_promises.py --write", file=sys.stderr)
        return 1

    print(f"README promise task list is current ({len(promises)} promises).")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
