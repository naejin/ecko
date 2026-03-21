#!/usr/bin/env python3
"""Session stats — standalone script for /ecko:session command."""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# Ensure the checks package is importable
sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from checks.config import get_session_hours, load_config
from checks.ledger import compute_self_corrections, read_session


def main() -> None:
    parser = argparse.ArgumentParser(description="Ecko session stats")
    parser.add_argument("--cwd", required=True, help="Project working directory")
    args = parser.parse_args()

    config = load_config(args.cwd)
    session_hours = get_session_hours(config)

    if session_hours <= 0:
        print("Session ledger is disabled (session_hours: 0).")
        return

    entries = read_session(args.cwd, session_hours=session_hours)
    if not entries:
        print("No session data yet.")
        return

    corrections = compute_self_corrections(entries)

    # Compute stats
    files: set[str] = set()
    total_echoes = 0
    check_counts: dict[str, int] = {}
    clean_first_pass = 0

    # Group by file for first-pass analysis
    by_file: dict[str, list[dict]] = {}
    for entry in entries:
        f = entry.get("file", "")
        if f:
            files.add(f)
            by_file.setdefault(f, []).append(entry)
        for check, count in entry.get("echoes", {}).items():
            total_echoes += count
            check_counts[check] = check_counts.get(check, 0) + count

    # Count files that were clean on first touch
    for file_entries in by_file.values():
        ordered = sorted(file_entries, key=lambda e: e.get("ts", 0))
        first = ordered[0]
        if not first.get("echoes", {}):
            clean_first_pass += 1

    total_corrected = sum(corrections.values())

    # Output
    print(f"~~ ecko session ~~ ({session_hours}h window)")
    print()
    print(f"  Files touched:      {len(files)}")
    print(f"  Total echoes:       {total_echoes}")
    print(f"  Self-corrected:     {total_corrected}")
    print(f"  Clean first pass:   {clean_first_pass}/{len(files)}")

    # Top checks
    if check_counts:
        print()
        print("  Top checks:")
        top = sorted(check_counts.items(), key=lambda x: -x[1])[:5]
        for check, count in top:
            print(f"    {check}: {count}")


if __name__ == "__main__":
    main()
