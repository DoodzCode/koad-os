#!/usr/bin/env python3
import sys
import subprocess
import json
import os
from pathlib import Path

def main():
    print("KoadOS Skill: Hello Koad")
    print("Arguments received:", sys.argv[1:])
    
    koad_home = Path(os.getenv("KOAD_HOME", Path.home() / ".koad-os"))
    koad_bin = koad_home / "bin" / "koad"

    # Example of a skill calling the koad CLI back
    try:
        result = subprocess.run(
            [str(koad_bin), "auth"],
            capture_output=True, text=True, check=True
        )
        print("Koad CLI Auth Response:")
        print(result.stdout.strip())
    except Exception as e:
        print(f"Failed to call koad CLI: {e}")

if __name__ == "__main__":
    main()
