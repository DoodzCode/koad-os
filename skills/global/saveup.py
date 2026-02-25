#!/usr/bin/env python3
import sys
import os
import subprocess
import argparse
from datetime import datetime
from pathlib import Path

KOAD_HOME = Path(os.getenv("KOAD_HOME", Path.home() / ".koad-os"))
KOAD_BIN = KOAD_HOME / "bin" / "koad"
LOG_FILE = KOAD_HOME / "SESSION_LOG.md"

def run_koad(args):
    try:
        subprocess.run([KOAD_BIN] + args, check=True, capture_output=True)
    except Exception as e:
        print(f"Error calling koad CLI: {e}")

def main():
    parser = argparse.ArgumentParser(description="KoadOS Unified Saveup Skill")
    parser.add_argument("--fact", action="append", help="A fact to remember")
    parser.add_argument("--learning", action="append", help="A lesson learned")
    parser.add_argument("--summary", required=True, help="Short summary of changes made")
    parser.add_argument("--scope", default="General", help="Scope of the work (e.g. project name)")
    
    args = parser.parse_args()

    print(f"--- KoadOS Saveup Initiated [{datetime.now().strftime('%Y-%m-%d %H:%M:%S')}] ---")

    # 1. Update Memory
    if args.fact:
        for f in args.fact:
            print(f"Remembering Fact: {f}")
            run_koad(["remember", "fact", f])
    
    if args.learning:
        for l in args.learning:
            print(f"Recording Learning: {l}")
            run_koad(["remember", "learning", l])

    # 2. Update Global Session Log
    log_path = Path(LOG_FILE)
    date_str = datetime.now().strftime("%Y-%m-%d")
    
    log_entry = f"""
## {date_str} - {args.summary}
- Scope: {args.scope}
"""
    if args.fact:
        log_entry += "- Facts added to memory.\n"
    if args.learning:
        log_entry += "- Learnings recorded.\n"
    
    try:
        with open(log_path, "a") as f:
            f.write(log_entry)
        print(f"Global Session Log updated: {log_path}")
    except Exception as e:
        print(f"Failed to update Session Log: {e}")

    print("--- Saveup Complete ---")

if __name__ == "__main__":
    main()
