#!/usr/bin/env bash
# sync-status.sh — Parse updates/ and report current phase status
# Usage: sync-status.sh [--write]

set -euo pipefail

KOAD_HOME="${KOAD_HOME:-$HOME/.koad-os}"
UPDATES_DIR="$KOAD_HOME/updates"
MISSION_FILE="$KOAD_HOME/MISSION.md"

if [[ ! -d "$UPDATES_DIR" ]]; then
  echo "[WARN] No updates/ directory found at $UPDATES_DIR"
  exit 0
fi

# Get most recent update file
LATEST=$(ls -1 "$UPDATES_DIR"/*.md 2>/dev/null | sort -r | head -1)
if [[ -z "$LATEST" ]]; then
  echo "[INFO] No update files found."
  exit 0
fi

# Extract frontmatter fields
extract_field() {
  local field="$1"
  local file="$2"
  grep -m1 "^${field}" "$file" | sed 's/.*= *"\(.*\)"/\1/'
}

SUMMARY=$(extract_field "summary" "$LATEST")
AUTHOR=$(extract_field "author" "$LATEST")
TIMESTAMP=$(extract_field "timestamp" "$LATEST" | cut -c1-10)
CATEGORY=$(extract_field "category" "$LATEST")

echo "=== KoadOS Status Sync ==="
echo "Latest Update: $TIMESTAMP [$CATEGORY] $SUMMARY — $AUTHOR"
echo ""
echo "Current Active Phase line in MISSION.md:"
grep "Active)" "$MISSION_FILE" || echo "(not found)"
echo ""

if [[ "${1:-}" == "--write" ]]; then
  # Update the Phase 7.5 active line — replace with latest summary
  sed -i "s|.*Phase.*Active.*|- **Phase 7.5 (Active):** $SUMMARY|" "$MISSION_FILE"
  echo "[OK] MISSION.md updated."
fi
