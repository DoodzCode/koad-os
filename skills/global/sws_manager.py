#!/usr/bin/env python3
import sys
import subprocess
import argparse
from pathlib import Path

import os

# Config
GCP_PROJECT = os.getenv("GCP_PROJECT", "default-project")
REGION = os.getenv("GCP_REGION", "us-central1")

def run_cmd(cmd, cwd=None):
    print(f"Executing: {' '.join(cmd)}")
    result = subprocess.run(cmd, cwd=cwd, text=True)
    if result.returncode != 0:
        print(f"Command failed with code {result.returncode}")
        sys.exit(1)

def main():
    parser = argparse.ArgumentParser(description="KoadOS SWS Deployment Dispatcher")
    parser.add_argument("action", choices=["run", "deploy", "list"])
    parser.add_argument("--name", help="Name of the function")
    
    args = parser.parse_args()
    base_dir = Path(os.getenv("SWS_FUNCTIONS_DIR", "."))

    if args.action == "list":
        print(f"Available SWS Functions in {base_dir.absolute()}:")
        if base_dir.exists():
            for d in base_dir.iterdir():
                if d.is_dir():
                    print(f"- {d.name}")
        return

    if not args.name:
        print("Error: --name is required for run/deploy")
        sys.exit(1)

    func_dir = base_dir / args.name
    if not func_dir.exists():
        print(f"Error: Function {args.name} not found in {base_dir}")
        sys.exit(1)

    if args.action == "run":
        print(f"Starting local runner for {args.name}...")
        # Note: In a real scenario, this would start functions-framework
        run_cmd(["npm", "start"], cwd=func_dir)

    elif args.action == "deploy":
        print(f"Deploying {args.name} to GCP...")
        # Example gcloud command (would need adjustment per function)
        deploy_cmd = [
            "gcloud", "functions", "deploy", args.name,
            "--runtime", "nodejs20",
            "--trigger-http",
            "--region", REGION,
            "--project", GCP_PROJECT,
            "--allow-unauthenticated"
        ]
        run_cmd(deploy_cmd, cwd=func_dir)

if __name__ == "__main__":
    main()
