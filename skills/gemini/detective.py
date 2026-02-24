#!/usr/bin/env python3
import sys
import subprocess
import os
import json
from pathlib import Path

def run_fd(pattern, path="."):
    try:
        result = subprocess.run(["fdfind", "--glob", pattern, "--max-depth", "5", "--no-ignore", path], capture_output=True, text=True, check=True)
        return result.stdout.strip().splitlines()
    except:
        return []

def get_architecture_summary(file_path):
    """
    Extracts high-level architectural signals (structs, classes, exports).
    """
    summary = []
    try:
        with open(file_path, 'r') as f:
            lines = f.readlines()
            for line in lines[:200]: # Only scan top of file for efficiency
                line = line.strip()
                # Rust Signals
                if file_path.endswith(".rs"):
                    if line.startswith("pub struct") or line.startswith("pub enum") or line.startswith("pub fn"):
                        summary.append(line.replace(" {", ""))
                # JS/TS Signals
                elif file_path.endswith((".js", ".ts")):
                    if line.startswith("export class") or line.startswith("export const") or line.startswith("export function"):
                        summary.append(line.replace(" {", ""))
    except:
        pass
    return summary[:5] # Limit to top 5 signals per file

def map_project(path="."):
    """
    Surgical Project Mapping with Architectural Intelligence.
    """
    print(f"--- Koad Detective: Mapping {os.path.abspath(path)} ---")
    
    report = {
        "identity": os.path.basename(os.path.abspath(path)),
        "core": {},
        "config": [],
        "tests": []
    }
    
    # 1. Detect Core Entry Points & Shape
    core_patterns = ["main.rs", "lib.rs", "mod.rs", "index.js", "index.ts", "tui.rs"]
    for p in core_patterns:
        found = run_fd(p, path)
        for f in found:
            report["core"][f] = get_architecture_summary(f)

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
