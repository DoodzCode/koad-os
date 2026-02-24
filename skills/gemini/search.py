#!/usr/bin/env python3
import sys
import subprocess
import json

def search_koad(query, limit=5):
    """
    Queries the KoadOS SQLite database with ranking and limit.
    """
    print(f"--- KoadOS Surgical Search: '{query}' (Limit: {limit}) ---")
    
    try:
        # Run koad query with limit
        result = subprocess.run(
            ["/home/ideans/.koad-os/bin/koad", "query", query, "--limit", str(limit)],
            capture_output=True,
            text=True,
            check=True
        )
        
        output = result.stdout.strip()
        if not output:
            print(f"No local matches found for '{query}'.")
            return None
            
        print(f"Found matches:\n{output}")
        return output

    except subprocess.CalledProcessError as e:
        print(f"Error querying KoadOS: {e}", file=sys.stderr)
        return None

if __name__ == "__main__":
    if len(sys.argv) < 2:
        print("Usage: koad skill run gemini/search.py -- "<query_term>"")
        sys.exit(1)
    
    search_term = sys.argv[1]
    search_koad(search_term)
