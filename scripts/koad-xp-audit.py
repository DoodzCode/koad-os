#!/usr/bin/env python3
import sys
import os
import re

def audit_ledger(file_path):
    if not os.path.exists(file_path):
        print(f"Error: Ledger file not found at {file_path}")
        return False

    with open(file_path, 'r') as f:
        lines = f.readlines()

    running_total = 0
    errors = 0
    
    print(f"--- Auditing Ledger: {file_path} ---")
    
    for i, line in enumerate(lines):
        # Look for table rows: | Date | ID | Event | Delta | Total | Level |
        if line.startswith('|') and 'Running Total' not in line and '---' not in line:
            parts = [p.strip() for p in line.split('|') if p.strip()]
            if len(parts) < 5:
                continue
                
            delta_str = parts[3]
            recorded_total_str = parts[4]
            
            try:
                # Handle deltas like +50 or -10
                delta = int(re.sub(r'[^\d\-]', '', delta_str))
                recorded_total = int(re.sub(r'[^\d\-]', '', recorded_total_str))
                
                expected_total = running_total + delta
                
                if expected_total != recorded_total:
                    print(f"Row {i+1}: Arithmetic Mismatch! Expected {expected_total}, found {recorded_total}")
                    errors += 1
                
                running_total = recorded_total
            except ValueError:
                print(f"Row {i+1}: Could not parse integers from '{delta_str}' or '{recorded_total_str}'")
                errors += 1

    if errors == 0:
        print(f"SUCCESS: Ledger arithmetic is valid. Final XP: {running_total}")
        return True
    else:
        print(f"FAILURE: {errors} errors found in ledger.")
        return False

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: koad-xp-audit.py <path_to_XP_LEDGER.md>")
        sys.exit(1)
    
    success = audit_ledger(sys.argv[1])
    sys.exit(0 if success else 1)
