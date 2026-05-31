#!/usr/bin/env bash
# =============================================================================
# KoadOS Distribution Sanitizer — "The Deep Scrub"
# Fulfills Task 4.3: Sanitize local Citadel to a Pure Distribution state.
# =============================================================================
set -euo pipefail

# ── Paths & Setup ────────────────────────────────────────────────────────────
# Scripts lives in scripts/ — KOAD_HOME is its parent directory
KOAD_HOME="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN_DIR="$KOAD_HOME/bin"

# ── Colours ───────────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

ok()   { echo -e "${GREEN}  ✓${RESET}  $*"; }
warn() { echo -e "${YELLOW}  ⚠${RESET}  $*"; }
fail() { echo -e "${RED}  ✗${RESET}  $*"; }
info() { echo -e "${CYAN}  →${RESET}  $*"; }

# ── Logic ─────────────────────────────────────────────────────────────────────

usage() {
    echo "KoadOS Distribution Sanitizer"
    echo "Usage: bash $0 [--confirm] [--full]"
    echo ""
    echo "Options:"
    echo "  --confirm    Mandatory flag to execute the scrub."
    echo "  --full       Additionally purges databases and Redis persistence."
    exit 1
}

# ── 1. Safeguard Checks ───────────────────────────────────────────────────────
CONFIRM=false
FULL=false

for arg in "$@"; do
    case $arg in
        --confirm) CONFIRM=true ;;
        --full) FULL=true ;;
        *) usage ;;
    esac
done

if [[ "$CONFIRM" != "true" ]]; then
    fail "Execution aborted. You MUST provide the --confirm flag."
    echo "WARNING: This script will delete ALL local instance data (logs, bays, history)."
    exit 1
fi

# Service check (check for active sockets)
info "Checking for active services..."
if [[ -S "$KOAD_HOME/run/koad.sock" ]] || [[ -S "$KOAD_HOME/run/kcitadel.sock" ]]; then
    fail "KoadOS sockets detected in run/. Please stop services before sanitizing."
    exit 1
fi

# ── 2. Mandatory Scrub ───────────────────────────────────────────────────────
echo -e "\n${BOLD}[Initiating Mandatory Scrub]${RESET}"

scrub_dir() {
    local target="$KOAD_HOME/$1"
    if [[ -d "$target" ]]; then
        info "Purging $1..."
        # We delete contents but keep the directory (though bootstrap will recreate it)
        find "$target" -mindepth 1 -delete
        ok "$1 cleaned"
    fi
}

scrub_dir "run"
scrub_dir "logs"
scrub_dir "cache"
scrub_dir "agents/bays"
scrub_dir "bays"
scrub_dir "agents/sessions_archive"

# Command History Scrub
info "Purging agent session history..."
find "$KOAD_HOME/agents/KAPVs" -name "bash_history" -type f -delete 2>/dev/null || true
ok "Bash history purged"

# ── 3. Optional Full Scrub ────────────────────────────────────────────────────
if [[ "$FULL" == "true" ]]; then
    echo -e "\n${BOLD}[Initiating Full Database Scrub]${RESET}"
    scrub_dir "data/db"
    scrub_dir "data/redis"
    
    # Remove top-level ephemeral files
    [[ -f "$KOAD_HOME/citadel.db" ]] && rm "$KOAD_HOME/citadel.db" && ok "citadel.db removed"
    [[ -f "$KOAD_HOME/cass.db" ]] && rm "$KOAD_HOME/cass.db" && ok "cass.db removed"
    [[ -f "$KOAD_HOME/codegraph.db" ]] && rm "$KOAD_HOME/codegraph.db" && ok "codegraph.db removed"
    [[ -f "$KOAD_HOME/redis.log" ]] && rm "$KOAD_HOME/redis.log" && ok "redis.log removed"
    [[ -f "$KOAD_HOME/.env" ]] && rm "$KOAD_HOME/.env" && ok ".env removed"
fi

# ── 4. Final Sweep (Redaction) ───────────────────────────────────────────────
echo -e "\n${BOLD}[Finalizing Sanctuary Redaction]${RESET}"

# Redact current_context.md
if [[ -f "$KOAD_HOME/current_context.md" ]]; then
    sed -i "s|/home/ideans/.koad-os|~/.koad-os|g" "$KOAD_HOME/current_context.md"
    ok "current_context.md redacted"
fi

# Redact TEAM-LOG.md (Recent Session)
if [[ -f "$KOAD_HOME/TEAM-LOG.md" ]]; then
    sed -i "s|/home/ideans/.koad-os|~/.koad-os|g" "$KOAD_HOME/TEAM-LOG.md"
    ok "TEAM-LOG.md redacted"
fi

echo -e "\n${BOLD}${GREEN}Deep Scrub Complete.${RESET}"
echo "The repository is now in a Pure Distribution state."
echo "Run 'bash install/bootstrap.sh' to re-initialize your instance."
echo
