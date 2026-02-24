#!/usr/bin/env python3
import sys
import subprocess
import os
import json
from pathlib import Path

def run_fd(pattern, path="."):
    try:
        result = subprocess.run(["fdfind", "--glob", pattern, "--max-depth", "3", "--no-ignore", path], capture_output=True, text=True, check=True)
        return result.stdout.strip().splitlines()
    except:
        return []

def map_project(path="."):
    """
    Surgical Project Mapping:
    Identifies entry points and config files with minimal token usage.
    """
    print(f"--- Koad Detective: Mapping {os.path.abspath(path)} ---")
    
    report = {
        "identity": os.path.basename(os.path.abspath(path)),
        "core": [],
        "config": [],
        "tests": []
    }
    
    # 1. Detect Core Entry Points
    core_patterns = ["*main.rs", "*index.js", "*app.py", "*main.c", "*index.ts"]
    for p in core_patterns:
        found = run_fd(p, path)
        if found: report["core"].extend(found[:2])

    # 2. Detect Configuration Files
    config_patterns = ["Cargo.toml", "package.json", "requirements.txt", "go.mod", "koad.json", "Dockerfile"]
    for p in config_patterns:
        found = run_fd(p, path)
        if found: report["config"].extend(found[:2])

    # 3. Detect Test Suites
    test_found = subprocess.run(["fdfind", "--glob", "tests", "--type", "d", "--max-depth", "2", "--no-ignore", path], capture_output=True, text=True).stdout.strip().splitlines()
    if test_found: report["tests"].extend(test_found[:1])

    # Output a compact, high-signal report
    print(json.dumps(report, indent=2))
    return report

if __name__ == "__main__":
    target = sys.argv[1] if len(sys.argv) > 1 else "."
    map_project(target)
