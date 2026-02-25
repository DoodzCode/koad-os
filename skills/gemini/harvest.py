#!/usr/bin/env python3
import sys
import subprocess
import os
from pathlib import Path

def run_koad_command(args):
    """Executes a koad CLI command."""
    koad_home = Path(os.getenv("KOAD_HOME", Path.home() / ".koad-os"))
    koad_bin = koad_home / "bin" / "koad"
    try:
        subprocess.run([str(koad_bin)] + args, check=True)
        return True
    except (subprocess.CalledProcessError, FileNotFoundError) as e:
        print(f"Error running koad command: {e}", file=sys.stderr)
        return False

def harvest_discoveries(findings):
    """
    Automates the 'saveup' and learning capture for a session.
    Fulfills the 'Consolidated Sync' directive by ensuring verified 
    discoveries reach the Koad Knowledge Base.
    """
    print(f"--- KoadOS Harvest: Gemini ---")
    
    # 1. Capture the primary finding
    koad_args = ["remember", "fact", f"Harvest: {findings}", "--tags", "harvest,gemini-session"]
    run_koad_command(koad_args)
    
    # 2. Trigger the global saveup to update logs and capture state
    print("Triggering session saveup...")
    run_koad_command(["saveup", findings])
    
    return True

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: koad skill run gemini/harvest.py -- "<findings_summary>"")
        sys.exit(1)
    
    findings_summary = sys.argv[1]
    if harvest_discoveries(findings_summary):
        print("Session harvest completed successfully.")
        sys.exit(0)
    else:
        sys.exit(1)
