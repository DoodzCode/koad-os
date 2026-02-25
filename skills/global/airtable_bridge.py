#!/usr/bin/env python3
import sys
import json
import subprocess
import argparse

def run_koad_airtable(args):
    cmd = ["koad", "airtable"] + args
    result = subprocess.run(cmd, capture_with_output=True, text=True)
    if result.returncode != 0:
        print(f"Error: {result.stderr}")
        sys.exit(1)
    return result.stdout

def main():
    parser = argparse.ArgumentParser(description="Gemini Bridge for Airtable Operations")
    subparsers = parser.add_subparsers(dest="command")

    # List command
    list_parser = subparsers.add_parser("list")
    list_parser.add_argument("--base-id", required=True)
    list_parser.add_argument("--table", required=True)
    list_parser.add_argument("--filter", help="Airtable formula filter")
    list_parser.add_argument("--limit", type=int, default=10)

    # Get command
    get_parser = subparsers.add_parser("get")
    get_parser.add_argument("--base-id", required=True)
    get_parser.add_argument("--table", required=True)
    get_parser.add_argument("--id", required=True)

    args = parser.parse_args()

    if args.command == "list":
        koad_args = ["list", args.base_id, args.table, "--limit", str(args.limit)]
        if args.filter:
            koad_args += ["--filter", args.filter]
        
        output = run_koad_airtable(koad_args)
        print(output)

    elif args.command == "get":
        koad_args = ["get", args.base_id, args.table, args.id]
        output = run_koad_airtable(koad_args)
        print(output)

if __name__ == "__main__":
    main()
