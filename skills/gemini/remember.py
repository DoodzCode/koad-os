#!/usr/bin/env python3
import sys
import json
import subprocess
import os
from pathlib import Path

def run_koad_command(args):
    """Executes a koad CLI command."""
    koad_home = Path(os.getenv("KOAD_HOME", Path.home() / ".koad-os"))
    koad_bin = koad_home / "bin" / "koad"
    try:
        # We don't use capture_output=True here to allow the user to see the koad output
        subprocess.run([str(koad_bin)] + args, check=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError) as e:
        print(f"Error running koad command: {e}", file=sys.stderr)
        return False

def bridge_memory(fact, tags=None):
    """
    Bridges memory between Gemini internal and KoadOS SQLite.
    
    Note: The 'save_memory' tool call must be handled by the Gemini agent itself.
    This script handles the KoadOS side and ensures structural consistency.
    """
    print(f"--- KoadOS Memory Bridge ---")
    print(f"Fact: {fact}")
    
    koad_args = ["remember", "fact", fact]
    if tags:
        koad_args.extend(["--tags", tags])
    
    success = run_koad_command(koad_args)
    
    if success:
        print("Successfully committed to KoadOS SQLite.")
    
    return success

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: koad skill run gemini/remember.py "<fact>" [tags]")
        sys.exit(1)
    
    fact_content = sys.argv[1]
    tags_content = sys.argv[2] if len(sys.argv) > 2 else "gemini-bridge"
    
    if bridge_memory(fact_content, tags_content):
        sys.exit(0)
    else:
        sys.exit(1)
