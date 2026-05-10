#!/usr/bin/env bash
# =============================================================================
# KoadOS Unified Stable Installer
# =============================================================================
set -euo pipefail

# Colours
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

ok()   { echo -e "${GREEN}  ✓${RESET}  $*"; }
warn() { echo -e "${YELLOW}  ⚠${RESET}  $*"; }
fail() { echo -e "${RED}  ✗${RESET}  $*"; }
info() { echo -e "${CYAN}  →${RESET}  $*"; }
section() { echo -e "\n${BOLD}[$*]${RESET}"; }

# Cleanup Logic
cleanup() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        echo -e "\n${RED}${BOLD}Installation failed at step: ${CURRENT_STEP:-Unknown}${RESET}"
        fail "Please resolve the issue and try again."
    fi
}
trap cleanup EXIT ERR INT TERM

echo -e "${BOLD}"
echo "  ██╗  ██╗ ██████╗  █████╗ ██████╗      ██████╗ ███████╗"
echo "  ██║ ██╔╝██╔═══██╗██╔══██╗██╔══██╗    ██╔═══██╗██╔════╝"
echo "  █████╔╝ ██║   ██║███████║██║  ██║    ██║   ██║███████╗"
echo "  ██╔═██╗ ██║   ██║██╔══██║██║  ██║    ██║   ██║╚════██║"
echo "  ██║  ██╗╚██████╔╝██║  ██║██████╔╝    ╚██████╔╝███████║"
echo "  ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═════╝      ╚═════╝ ╚══════╝"
echo -e "${RESET}"
echo "  Unified Stable Installer  ·  v3.2.0"
echo

# 0. Global Configuration
CURRENT_STEP="Configuration"
KOAD_HOME="${KOADOS_HOME:-$HOME/.koad-os}"

# 1. Prerequisite Detection
CURRENT_STEP="Prerequisite Detection"
section "Prerequisite Detection"
info "KoadOS Target Instance: $KOAD_HOME"
ERRORS=0

check_cmd() {
    local cmd=$1
    local msg=${2:-$1}
    if command -v "$cmd" &>/dev/null; then
        ok "$msg found ($(command -v "$cmd"))"
    else
        fail "$msg not found. Please install it to continue."
        ERRORS=$((ERRORS + 1))
    fi
}

check_cmd "rustc" "Rust Compiler (rustc)"
check_cmd "cargo" "Rust Package Manager (cargo)"
check_cmd "docker" "Docker"
check_cmd "protoc" "Protocol Buffers Compiler (protoc)"

# Check for docker-compose or docker compose
if command -v "docker-compose" &>/dev/null; then
    ok "docker-compose found"
elif docker compose version &>/dev/null; then
    ok "docker compose plugin found"
else
    fail "docker-compose or docker compose plugin not found."
    ERRORS=$((ERRORS + 1))
fi

check_cmd "python3" "Python 3"
check_cmd "pipx" "pipx"

if [[ $ERRORS -gt 0 ]]; then
    echo
    fail "$ERRORS prerequisite(s) missing. Fix the above and re-run $0."
    exit 1
fi

# 2. Install Graph Tools
CURRENT_STEP="Graph Tools Installation"
section "Graph Tools Installation"
if command -v "code-review-graph" &>/dev/null; then
    ok "code-review-graph already installed ($(command -v "code-review-graph"))"
else
    info "Installing code-review-graph via pipx..."
    pipx install code-review-graph
    ok "code-review-graph installed."
fi

# 3. Infrastructure Boot
CURRENT_STEP="Infrastructure Boot"
section "Infrastructure Boot (Docker)"
info "Starting CASS, Redis, and Qdrant..."
if command -v "docker-compose" &>/dev/null; then
    docker-compose up -d --build
else
    docker compose up -d --build
fi
ok "Infrastructure is running in the background."

# 4. Host Binary Compilation
CURRENT_STEP="Binary Compilation"
section "Host Binary Compilation (Rust)"
info "Building 'koad' and 'koad-agent' in release mode..."
cargo build --release --bin koad --bin koad-agent
ok "Binaries compiled successfully."

# 5. Next Steps
CURRENT_STEP="Finalizing"
section "Installation Phase 1 Complete"
info "Infrastructure is online and binaries are built."
echo -e "\n${BOLD}Next Step:${RESET}"
echo -e "Run the initialization script to set up your Citadel identity:"
echo -e "  ${CYAN}./koad-init.sh${RESET}\n"
