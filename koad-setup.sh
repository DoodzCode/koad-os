#!/usr/bin/env bash
# =============================================================================
# KoadOS WSL Bootstrap — Install All Prerequisites
# Run this FIRST on a fresh Ubuntu/WSL system.
# After completion, run: ./install.sh then ./koad-init.sh
# =============================================================================
set -euo pipefail

RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

ok()      { echo -e "${GREEN}  ✓${RESET}  $*"; }
warn()    { echo -e "${YELLOW}  ⚠${RESET}  $*"; }
fail()    { echo -e "${RED}  ✗${RESET}  $*"; }
info()    { echo -e "${CYAN}  →${RESET}  $*"; }
section() { echo -e "\n${BOLD}[$*]${RESET}"; }

echo -e "${BOLD}"
echo "  KoadOS WSL Bootstrap"
echo "  Installs: Rust, Docker, protoc, pipx, rtk"
echo -e "${RESET}"

# 1. System Packages
section "System Packages"
info "Updating apt and installing base packages..."
sudo apt-get update -qq
sudo apt-get install -y \
    curl \
    ca-certificates \
    gnupg \
    lsb-release \
    protobuf-compiler \
    python3 \
    python3-pip \
    pipx
ok "System packages installed."
pipx ensurepath

# 2. Rust
section "Rust Toolchain"
if command -v rustc &>/dev/null; then
    ok "Rust already installed: $(rustc --version)"
else
    info "Installing Rust via rustup..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path
    ok "Rust installed."
fi
# Ensure cargo is available in this session
source "$HOME/.cargo/env" 2>/dev/null || true
export PATH="$HOME/.cargo/bin:$PATH"

# 3. Docker Engine
section "Docker Engine"
if command -v docker &>/dev/null; then
    ok "Docker already installed: $(docker --version)"
else
    info "Installing Docker Engine (Ubuntu)..."
    sudo install -m 0755 -d /etc/apt/keyrings
    curl -fsSL https://download.docker.com/linux/ubuntu/gpg \
        | sudo gpg --dearmor -o /etc/apt/keyrings/docker.gpg
    sudo chmod a+r /etc/apt/keyrings/docker.gpg
    echo \
        "deb [arch=$(dpkg --print-architecture) signed-by=/etc/apt/keyrings/docker.gpg] \
https://download.docker.com/linux/ubuntu \
$(. /etc/os-release && echo "$VERSION_CODENAME") stable" \
        | sudo tee /etc/apt/sources.list.d/docker.list > /dev/null
    sudo apt-get update -qq
    sudo apt-get install -y \
        docker-ce \
        docker-ce-cli \
        containerd.io \
        docker-buildx-plugin \
        docker-compose-plugin
    sudo usermod -aG docker "$USER"
    ok "Docker Engine installed."
    warn "Docker group added. You may need to log out and back in (or run: newgrp docker)."
fi

# 4. rtk (Rust Token Killer)
section "rtk (Token Compression)"
if command -v rtk &>/dev/null; then
    ok "rtk already installed: $(rtk --version 2>/dev/null || echo 'installed')"
else
    info "Installing rtk..."
    curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh
    export PATH="$HOME/.local/bin:$PATH"
    ok "rtk installed to ~/.local/bin"
fi

# 5. Shell Config
section "Shell Configuration"
SHELL_RC="$HOME/.bashrc"
[[ "$SHELL" == */zsh ]] && SHELL_RC="$HOME/.zshrc"

CARGO_LINE='export PATH="$HOME/.cargo/bin:$PATH"'
LOCAL_LINE='export PATH="$HOME/.local/bin:$PATH"'

if ! grep -q '\.cargo/bin' "$SHELL_RC" 2>/dev/null; then
    echo "$CARGO_LINE" >> "$SHELL_RC"
    ok "Added Rust to $SHELL_RC"
fi
if ! grep -q '\.local/bin' "$SHELL_RC" 2>/dev/null; then
    echo "$LOCAL_LINE" >> "$SHELL_RC"
    ok "Added ~/.local/bin to $SHELL_RC"
fi

# 6. Summary
section "Bootstrap Complete"
ok "All prerequisites installed."
echo ""
echo -e "${BOLD}Next Steps:${RESET}"
echo -e "1. If Docker group was added, log out and back in (or run: ${CYAN}newgrp docker${RESET})"
echo -e "2. Reload your shell: ${CYAN}source $SHELL_RC${RESET}"
echo -e "3. Build the Citadel:  ${CYAN}./install.sh${RESET}"
echo -e "4. Initialize:         ${CYAN}./koad-init.sh${RESET}"
echo ""
