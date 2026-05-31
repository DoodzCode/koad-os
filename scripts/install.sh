#!/usr/bin/env bash
# =============================================================================
# KoadOS Installer — v3.2.0 Stable
# =============================================================================
set -euo pipefail

KOAD_HOME="${KOAD_HOME:-$HOME/.koad-os}"
BIN_DIR="$KOAD_HOME/bin"
LOG_DIR="$KOAD_HOME/logs"

# Colours
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

ok()   { echo -e "${GREEN}  ✓${RESET}  $*"; }
warn() { echo -e "${YELLOW}  ⚠${RESET}  $*"; }
fail() { echo -e "${RED}  ✗${RESET}  $*"; }
info() { echo -e "${CYAN}  →${RESET}  $*"; }
section() { echo -e "\n${BOLD}[$*]${RESET}"; }

echo -e "${BOLD}"
echo "  ██╗  ██╗ ██████╗  █████╗ ██████╗      ██████╗ ███████╗"
echo "  ██║ ██╔╝██╔═══██╗██╔══██╗██╔══██╗    ██╔═══██╗██╔════╝"
echo "  █████╔╝ ██║   ██║███████║██║  ██║    ██║   ██║███████╗"
echo "  ██╔═██╗ ██║   ██║██╔══██║██║  ██║    ██║   ██║╚════██║"
echo "  ██║  ██╗╚██████╔╝██║  ██║██████╔╝    ╚██████╔╝███████║"
echo "  ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═════╝      ╚═════╝ ╚══════╝"
echo -e "${RESET}"
echo "  KoadOS Stable Installer  ·  v3.2.0"
echo

# 1. Prerequisite Detection
section "Prerequisite Detection"
ERRORS=0

check_cmd() {
    if command -v "$1" &>/dev/null; then
        ok "$1 found ($(command -v "$1"))"
    else
        fail "$1 not found. Run: $2"
        ERRORS=$((ERRORS + 1))
    fi
}

check_cmd "rustc"      "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
check_cmd "cargo"      "See above for rustup"
check_cmd "docker"     "sudo apt install docker.io && sudo usermod -aG docker \$USER"
check_cmd "sqlite3"    "sudo apt install sqlite3"
check_cmd "protoc"     "sudo apt install protobuf-compiler"
check_cmd "redis-server" "sudo apt install redis-server"

if [[ $ERRORS -gt 0 ]]; then
    echo
    fail "$ERRORS prerequisite(s) missing. Fix the above and re-run $0."
    exit 1
fi

# 2. Directory Setup
section "Directory Setup"
for dir in "$BIN_DIR" "$LOG_DIR" "$KOAD_HOME/cache" "$KOAD_HOME/data/db" "$KOAD_HOME/data/redis" "$KOAD_HOME/run" "$KOAD_HOME/config/integrations"; do
    mkdir -p "$dir"
    ok "$dir created"
done

# 3. Interactive .env Generator
section "Environment Setup"
if [[ -f "$KOAD_HOME/.env" ]]; then
    ok ".env already exists"
else
    echo "Let's set up your first AI Provider (you can skip and edit .env later)."
    read -p "Choose provider [gemini/claude/codex/none]: " PROVIDER
    
    cp .env.template "$KOAD_HOME/.env"
    
    if [[ "$PROVIDER" == "gemini" ]]; then
        read -sp "Enter Google AI (Gemini) API Key: " API_KEY
        echo ""
        sed -i "s/GOOGLE_AI_API_KEY=.*/GOOGLE_AI_API_KEY=$API_KEY/" "$KOAD_HOME/.env"
        ok "Gemini key saved to .env"
    elif [[ "$PROVIDER" == "claude" ]]; then
        read -sp "Enter Anthropic (Claude) API Key: " API_KEY
        echo ""
        sed -i "s/ANTHROPIC_API_KEY=.*/ANTHROPIC_API_KEY=$API_KEY/" "$KOAD_HOME/.env"
        ok "Claude key saved to .env"
    elif [[ "$PROVIDER" == "codex" ]]; then
        read -sp "Enter OpenAI API Key: " API_KEY
        echo ""
        sed -i "s/OPENAI_API_KEY=.*/OPENAI_API_KEY=$API_KEY/" "$KOAD_HOME/.env"
        ok "OpenAI key saved to .env"
    else
        warn "No provider key set. You will need to edit $KOAD_HOME/.env manually."
    fi
fi

# 4. Configuration Hydration
section "Configuration Hydration"
KOAD_HOME_ESCAPED=$(echo "$KOAD_HOME" | sed 's/\//\\\//g')

if [[ ! -f "$KOAD_HOME/config/kernel.toml" ]]; then
    cp config/defaults/kernel.toml "$KOAD_HOME/config/kernel.toml"
    ok "config/kernel.toml initialized"
fi

if [[ ! -f "$KOAD_HOME/run/redis.active.conf" ]]; then
    # Hydrate the Redis config template with resolved KOAD_HOME
    mkdir -p "$KOAD_HOME/run"
    sed "s/{{KOAD_HOME}}/$KOAD_HOME_ESCAPED/g" \
        config/defaults/redis.conf.template > "$KOAD_HOME/run/redis.active.conf"
    ok "run/redis.active.conf initialized with $KOAD_HOME"
fi

# 5. Compile & Install
section "Compiling KoadOS (Release Mode)"
cargo build --release --bin koad --bin koad-agent --bin koad-citadel --bin koad-cass

for bin in koad koad-agent koad-citadel koad-cass; do
    cp "target/release/$bin" "$BIN_DIR/$bin"
    ok "$bin installed to $BIN_DIR"
done

# 6. Shell Integration
section "Shell Integration"
BASHRC="$HOME/.bashrc"
if ! grep -q "koad-functions.sh" "$BASHRC"; then
    echo "source $KOAD_HOME/bin/koad-functions.sh" >> "$BASHRC"
    echo "export PATH=\"$BIN_DIR:\$PATH\"" >> "$BASHRC"
    ok "Added KoadOS to $BASHRC"
    info "Run 'source ~/.bashrc' after this script finishes."
fi

ok "Installation complete! Run 'koad system doctor' to verify."
