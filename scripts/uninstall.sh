#!/usr/bin/env bash
# =============================================================================
# KoadOS Uninstaller
# =============================================================================
# Prevent sourcing the script to avoid closing parent terminal on exits/traps
if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
    echo -e "\033[0;31m✗\033[0m Error: This script must be executed directly, not sourced."
    echo "Run it as: ./uninstall.sh [KOAD_HOME]"
    return 1 2>/dev/null || exit 1
fi

set -euo pipefail

# Colours
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

ok()   { echo -e "${GREEN}  ✓${RESET}  $*"; }
warn() { echo -e "${YELLOW}  ⚠${RESET}  $*"; }
fail() { echo -e "${RED}  ✗${RESET}  $*"; }
info() { echo -e "${CYAN}  →${RESET}  $*"; }
section() { echo -e "\n${BOLD}[$*]${RESET}"; }

# Find Repo Root
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Argument Parsing
FORCE=false
POSITIONAL_ARGS=()
while [[ $# -gt 0 ]]; do
  case $1 in
    --force)
      FORCE=true
      shift
      ;;
    -h|--help)
      echo "Usage: $0 [KOAD_HOME] [--force]"
      exit 0
      ;;
    *)
      POSITIONAL_ARGS+=("$1")
      shift
      ;;
  esac
done

section "KoadOS Uninstallation"

# 1. Detect KOADOS_HOME
KOAD_HOME="${KOADOS_HOME:-${POSITIONAL_ARGS[0]:-$HOME/.koad-os}}"
info "KoadOS Home detected: $KOAD_HOME"

# 2. Stop Infrastructure
section "Stopping Infrastructure"
cd "$REPO_ROOT"
if [[ -f "docker-compose.yml" ]]; then
    info "Stopping Docker containers and removing volumes..."
    if command -v "docker-compose" &>/dev/null; then
        docker-compose down -v || true
    else
        docker compose down -v || true
    fi
    ok "Infrastructure stopped."
else
    warn "docker-compose.yml not found in $REPO_ROOT. Skipping Docker cleanup."
fi

# 3. Remove KOADOS_HOME
section "Removing KoadOS Files"
if [[ -d "$KOAD_HOME" ]]; then
    SHOULD_REMOVE=false
    if [[ "$FORCE" = true ]]; then
        SHOULD_REMOVE=true
    elif [[ -t 0 ]]; then
        echo -e "${YELLOW}${BOLD}WARNING: This will delete ALL data in $KOAD_HOME${RESET}"
        read -p "Are you sure you want to continue? [y/N] " confirm
        if [[ "$confirm" =~ ^[Yy]$ ]]; then
            SHOULD_REMOVE=true
        fi
    else
        warn "Non-interactive shell detected and --force not used. Skipping $KOAD_HOME removal."
    fi

    if [[ "$SHOULD_REMOVE" = true ]]; then
        rm -rf "$KOAD_HOME"
        ok "$KOAD_HOME has been removed."
    else
        info "Preserving $KOAD_HOME."
    fi
else
    ok "$KOAD_HOME directory does not exist. Nothing to remove."
fi

section "Uninstallation Summary"
info "KoadOS binaries and infrastructure have been processed."
echo -e "\n${BOLD}Manual Actions Required:${RESET}"
echo -e "1. Remove KoadOS entries from your shell profile (e.g., ~/.bashrc):"
echo -e "   ${CYAN}KOADOS_HOME, PATH additions, and koad-functions.sh sourcing${RESET}"
echo -e "2. Remove any remaining Docker images if desired:"
echo -e "   ${CYAN}docker images | grep koados${RESET}\n"
