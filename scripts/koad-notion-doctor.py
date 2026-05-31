#!/usr/bin/env python3
import os
import sys
import json
import subprocess
import time

# --- Configuration ---
NOTION_PAGE_ID = "30cfe8ec-ae8f-808b-8eff-fa75e1cb0572"  # KoadOS Core Contract
REQUIRED_ENV = ["NOTION_PAT", "KOADOS_PAT_NOTION_MAIN"]

def print_status(label, status, message=""):
    color = "\033[32m[PASS]\033[0m" if status == "PASS" else \
            "\033[31m[FAIL]\033[0m" if status == "FAIL" else \
            "\033[33m[WARN]\033[0m"
    print(f"{label:<35} {color} {message}")

def check_env():
    print("\033[1m--- [PHASE 1] Credential Audit ---\033[0m")
    all_pass = True
    for var in REQUIRED_ENV:
        val = os.environ.get(var)
        if val:
            # Mask the token for safety
            masked = val[:7] + "..." + val[-4:] if len(val) > 10 else "***"
            print_status(f"Env: {var}", "PASS", f"Value detected ({masked})")
        else:
            print_status(f"Env: {var}", "FAIL", "Variable missing from environment.")
            all_pass = False
    return all_pass

def check_config():
    print("\n\033[1m--- [PHASE 2] Integration Config ---\033[0m")
    config_path = os.path.expanduser("~/.koad-os/config/integrations/notion.toml")
    if os.path.exists(config_path):
        print_status("Config: notion.toml", "PASS", f"Found at {config_path}")
        return True
    else:
        print_status("Config: notion.toml", "FAIL", "Configuration file missing.")
        return False

def check_api():
    print("\n\033[1m--- [PHASE 3] Live API Handshake ---\033[0m")
    start_time = time.time()
    try:
        # We must ensure KOADOS_PAT_NOTION_MAIN is set for the koad command
        env = os.environ.copy()
        if "NOTION_PAT" in env and "KOADOS_PAT_NOTION_MAIN" not in env:
             env["KOADOS_PAT_NOTION_MAIN"] = env["NOTION_PAT"]

        result = subprocess.run(
            ["koad", "bridge", "notion", "read", NOTION_PAGE_ID],
            capture_output=True,
            text=True,
            env=env
        )
        duration = int((time.time() - start_time) * 1000)

        if result.returncode == 0 and "KoadOS Core Contract" in result.stdout:
            print_status("Notion API: Page Read", "PASS", f"Verified in {duration}ms")
            return True
        else:
            error_msg = result.stderr.strip().split('\n')[-1] if result.stderr else "Unknown error"
            print_status("Notion API: Page Read", "FAIL", f"Error: {error_msg}")
            return False
    except Exception as e:
        print_status("Notion API: Page Read", "FAIL", f"Exception: {str(e)}")
        return False

def main():
    print("\033[1;34m══════════════ [ NOTION DIAGNOSTICS ] ══════════════\033[0m")
    e = check_env()
    c = check_config()
    a = check_api()
    
    print("\033[1;34m═════════════════════════════════════════════════════\033[0m")
    if e and c and a:
        print("\033[32m[SYSTEM READY]\033[0m Notion tools are fully functional.")
        sys.exit(0)
    else:
        print("\033[31m[SYSTEM IMPAIRED]\033[0m One or more checks failed. Review logs above.")
        sys.exit(1)

if __name__ == "__main__":
    main()
