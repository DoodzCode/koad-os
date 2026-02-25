#!/usr/bin/env python3
import sys
import subprocess
import argparse
from pathlib import Path
import os

def run_koad(args):
    koad_home = Path(os.getenv("KOAD_HOME", Path.home() / ".koad-os"))
    koad_bin = koad_home / "bin" / "koad"
    try:
        subprocess.run([str(koad_bin)] + args, check=True)
    except Exception as e:
        print(f"Error calling koad CLI: {e}")

def main():
    parser = argparse.ArgumentParser(description="KoadOS Partner Tutor")
    parser.add_argument("question", help="The question or topic you need help with")
    
    args = parser.parse_args()
    
    print(f"--- KoadOS Partner Tutor: Analyzing '{args.question}' ---")
    
    # Simple keyword-based routing for a support skill
    q = args.question.lower()
    
    if "skill" in q:
        print("Tip: New skills should be placed in ~/.koad-os/skills/global/ (or gemini/ for model-specific logic).")
        print("Example: Copy skills/global/hello_koad.py to get started.")
        run_koad(["guide", "development"])
    elif "boot" in q or "onboard" in q:
        print("Tip: Run 'koad boot' at the start of every session to establish context.")
        run_koad(["guide", "onboarding"])
    elif "architecture" in q or "spine" in q:
        print("Tip: The Spine (koad-daemon) handles background delta tracking.")
        run_koad(["guide", "architecture"])
    elif "sync" in q or "notion" in q:
        print("Tip: Ensure NOTION_TOKEN and KOAD_STREAM_DB_ID are set in your environment.")
        print("Run 'koad diagnostic --full' to check your connection.")
    else:
        print("I'm not sure about that specific topic yet.")
        print("Try: 'koad guide' to see all documentation.")

if __name__ == "__main__":
    main()
