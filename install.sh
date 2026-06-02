#!/usr/bin/env bash
# =============================================================================
# KoadOS Unified Installer & Updater
# =============================================================================
# Prevent sourcing the script to avoid closing parent terminal on exits/traps
if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
    echo -e "\033[0;31m✗\033[0m Error: This script must be executed directly, not sourced."
    echo "Run it as: ./install.sh or bash install.sh"
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

# Cleanup Logic
cleanup() {
    local exit_code=$?
    if [[ $exit_code -ne 0 ]]; then
        echo -e "\n${RED}${BOLD}Execution failed at step: ${CURRENT_STEP:-Unknown}${RESET}"
        fail "Please resolve the issue and try again."
    fi
}
trap cleanup EXIT ERR INT TERM

portable_sed() {
    local pattern="$1"
    local file="$2"
    local tmpfile
    tmpfile=$(mktemp)
    sed "$pattern" "$file" > "$tmpfile" && mv "$tmpfile" "$file"
}

# Locate existing Citadels
locate_citadels() {
    local found=()
    
    # 1. Check KOADOS_HOME environment variable
    if [[ -n "${KOADOS_HOME:-}" && -d "$KOADOS_HOME" ]]; then
        found+=("$KOADOS_HOME")
    fi
    
    # 2. Check systemd service koad-citadel
    if command -v systemctl &>/dev/null; then
        local sys_path_citadel sys_path_cass
        sys_path_citadel=$(systemctl show koad-citadel.service -p WorkingDirectory 2>/dev/null | cut -d= -f2 || true)
        if [[ -n "$sys_path_citadel" && -d "$sys_path_citadel" ]]; then
            found+=("$sys_path_citadel")
        fi
        sys_path_cass=$(systemctl show koad-cass.service -p WorkingDirectory 2>/dev/null | cut -d= -f2 || true)
        if [[ -n "$sys_path_cass" && -d "$sys_path_cass" ]]; then
            found+=("$sys_path_cass")
        fi
    fi
    
    # 3. Check active directory for Citadel Jupiter
    if [[ -d "$HOME/.citadel-jupiter" ]]; then
        found+=("$HOME/.citadel-jupiter")
    fi
    
    # 4. Check default ~/.koad-os
    if [[ -d "$HOME/.koad-os" ]]; then
        found+=("$HOME/.koad-os")
    fi

    # Deduplicate and resolve real paths
    local uniq_found=()
    for path in "${found[@]}"; do
        local abs_path
        abs_path=$(realpath "$path" 2>/dev/null || echo "$path")
        if [[ -d "$abs_path" ]]; then
            local duplicate=false
            for u in "${uniq_found[@]}"; do
                if [[ "$u" == "$abs_path" ]]; then
                    duplicate=true
                    break
                fi
            done
            if [[ "$duplicate" == false ]]; then
                uniq_found+=("$abs_path")
            fi
        fi
    done

    echo "${uniq_found[@]:-}"
}

# -----------------------------------------------------------------------------
# Update Mode
# -----------------------------------------------------------------------------
run_update() {
    CURRENT_STEP="Update Setup"
    section "Locating Locally Installed Citadels"
    
    local paths
    paths=($(locate_citadels))
    
    if [[ ${#paths[@]} -eq 0 ]]; then
        fail "No existing Citadel installations located on this system."
        info "To perform a clean installation, run: ./install.sh --install"
        exit 1
    fi
    
    info "Found ${#paths[@]} installation(s) to update:"
    for p in "${paths[@]}"; do
        ok "  -> $p"
    done
    
    # Compilation
    CURRENT_STEP="Compilation"
    section "Compiling KoadOS Release Binaries"
    cargo build --release
    ok "Compilation complete."
    
    for p in "${paths[@]}"; do
        CURRENT_STEP="Updating Citadel"
        section "Updating Citadel at $p"
        
        # 1. Stop systemd services if active
        local restart_citadel=false
        local restart_cass=false
        if command -v systemctl &>/dev/null; then
            if systemctl is-active --quiet koad-citadel.service; then
                info "Stopping koad-citadel.service..."
                if sudo -n systemctl stop koad-citadel.service 2>/dev/null; then
                    restart_citadel=true
                else
                    warn "Could not stop koad-citadel.service (sudo password required). Proceeding with live replacement."
                fi
            fi
            if systemctl is-active --quiet koad-cass.service; then
                info "Stopping koad-cass.service..."
                if sudo -n systemctl stop koad-cass.service 2>/dev/null; then
                    restart_cass=true
                else
                    warn "Could not stop koad-cass.service (sudo password required). Proceeding with live replacement."
                fi
            fi
        fi
        
        # 2. Copy binaries
        local bin_dir="$p/bin"
        mkdir -p "$bin_dir"
        local bins=(koad koad-agent koad-cass koad-citadel koad-fs-mcp koad-map koad-notion-mcp koad-os-mcp invoke-tool register-tool cass-ingest koad-mcp)
        for bin in "${bins[@]}"; do
            if [[ -f "target/release/$bin" ]]; then
                # Remove target first to avoid "text file busy" errors
                rm -f "$bin_dir/$bin"
                cp "target/release/$bin" "$bin_dir/$bin"
                ok "  ✓ Updated binary: $bin"
            fi
        done
        
        # 3. Copy scripts / helpers
        if [[ -f "scripts/koad-functions.sh" ]]; then
            cp "scripts/koad-functions.sh" "$bin_dir/koad-functions.sh"
            ok "  ✓ Updated koad-functions.sh"
        fi
        if [[ -f "plugin/bin/agent-boot.sh" ]]; then
            cp "plugin/bin/agent-boot.sh" "$bin_dir/agent-boot.sh"
            ok "  ✓ Updated agent-boot.sh"
        fi
        
        # 4. Copy scripts folder
        if [[ -d "scripts" ]]; then
            mkdir -p "$p/scripts"
            cp -r scripts/. "$p/scripts/"
            ok "  ✓ Updated scripts directory"
        fi
        
        # 5. Copy skills
        if [[ -d "plugin/skills" ]]; then
            mkdir -p "$p/skills"
            cp -r plugin/skills/. "$p/skills/"
            ok "  ✓ Updated skills"
        fi
        
        # 6. Copy docker rook assets
        if [[ -d "docker/rook" ]]; then
            mkdir -p "$p/docker/rook"
            cp -r docker/rook/. "$p/docker/rook/"
            ok "  ✓ Updated docker/rook assets"
        fi
        
        # 7. Restart systemd services if they were active
        if [[ "$restart_cass" = true ]]; then
            info "Restarting koad-cass.service..."
            sudo -n systemctl start koad-cass.service || warn "Could not restart koad-cass.service."
        fi
        if [[ "$restart_citadel" = true ]]; then
            info "Restarting koad-citadel.service..."
            sudo -n systemctl start koad-citadel.service || warn "Could not restart koad-citadel.service."
        fi
        
        ok "Citadel update at $p complete!"
    done
    
    CURRENT_STEP="Finalizing"
    section "All updates complete!"
    ok "Citadel installations have been successfully updated to version 3.2.0."
}

# -----------------------------------------------------------------------------
# Clean Install Mode
# -----------------------------------------------------------------------------
run_install() {
    # 0. Global Configuration
    CURRENT_STEP="Configuration Setup"
    KOAD_HOME="${KOAD_HOME:-${KOADOS_HOME:-$HOME/.koad-os}}"
    BIN_DIR="$KOAD_HOME/bin"
    LOG_DIR="$KOAD_HOME/logs"
    
    # 0b. Check for Older Versions and Clean Up
    if [[ -d "$KOAD_HOME" || -f "/etc/systemd/system/koad-citadel.service" ]]; then
        warn "An existing Citadel installation was detected."
        local clean_choice="n"
        if [[ -t 0 ]]; then
            read -p "  Would you like to perform a clean installation (deletes older installation)? [y/N]: " clean_choice
        fi
        if [[ "$clean_choice" =~ ^[Yy]$ ]]; then
            info "Cleaning up older installation..."
            if [[ -f "scripts/uninstall.sh" ]]; then
                ./scripts/uninstall.sh "$KOAD_HOME" --force || true
            fi
            sudo -n systemctl stop koad-citadel.service koad-cass.service 2>/dev/null || true
            sudo -n systemctl disable koad-citadel.service koad-cass.service 2>/dev/null || true
            sudo rm -f /etc/systemd/system/koad-citadel.service /etc/systemd/system/koad-cass.service
            sudo systemctl daemon-reload
            ok "Clean up complete."
        else
            info "Proceeding with standard install (existing files will be overwritten)."
        fi
    fi

    # 0c. Prompt for Citadel Name & Captain Agent Name
    local citadel_name="Sanctuary"
    local captain_name="Tyr"
    if [[ -t 0 ]]; then
        section "Citadel Configuration"
        read -p "  Enter your Citadel Name [Sanctuary]: " user_citadel
        citadel_name=${user_citadel:-Sanctuary}
        read -p "  Enter your Captain Agent Name [Tyr]: " user_captain
        captain_name=${user_captain:-Tyr}
    fi
    
    # 1. Prerequisite Detection
    CURRENT_STEP="Prerequisite Detection"
    section "Prerequisite Detection"
    info "KoadOS Target Instance: $KOAD_HOME"
    local errors=0
    
    check_cmd() {
        local cmd=$1
        local msg=${2:-$1}
        if command -v "$cmd" &>/dev/null; then
            ok "$msg found ($(command -v "$cmd"))"
        else
            fail "$msg not found. Please install it to continue."
            errors=$((errors + 1))
        fi
    }
    
    check_cmd "rustc" "Rust Compiler (rustc)"
    check_cmd "cargo" "Rust Package Manager (cargo)"
    check_cmd "docker" "Docker"
    check_cmd "protoc" "Protocol Buffers Compiler (protoc)"
    check_cmd "sqlite3" "SQLite3"
    check_cmd "redis-server" "Redis Server"
    check_cmd "python3" "Python 3"
    check_cmd "pipx" "pipx"
    
    # Check for docker-compose or docker compose
    if command -v "docker-compose" &>/dev/null; then
        ok "docker-compose found"
    elif docker compose version &>/dev/null; then
        ok "docker compose plugin found"
    else
        fail "docker-compose or docker compose plugin not found."
        errors=$((errors + 1))
    fi
    
    if [[ $errors -gt 0 ]]; then
        echo
        fail "$errors prerequisite(s) missing. Fix them and re-run this installer."
        info "Tip: You can use './koad-setup.sh' to install system prerequisites first."
        exit 1
    fi
    
    # 2. Directory Setup
    CURRENT_STEP="Directory Setup"
    section "Directory Setup"
    for dir in "$BIN_DIR" "$LOG_DIR" "$KOAD_HOME/cache" "$KOAD_HOME/data/db" "$KOAD_HOME/data/redis" "$KOAD_HOME/run" "$KOAD_HOME/config" "$KOAD_HOME/config/identities" "$KOAD_HOME/skills" "$KOAD_HOME/docker/rook"; do
        mkdir -p "$dir"
        ok "$dir created"
    done
    
    # 3. Graph Tools & RTK
    CURRENT_STEP="Core Utilities Setup"
    section "Core CLI Utilities"
    if command -v "code-review-graph" &>/dev/null; then
        ok "code-review-graph already installed"
    else
        info "Installing code-review-graph via pipx..."
        pipx install code-review-graph || true
    fi
    
    if command -v "rtk" &>/dev/null; then
        ok "rtk already installed"
    else
        info "Installing rtk..."
        curl -fsSL https://raw.githubusercontent.com/rtk-ai/rtk/refs/heads/master/install.sh | sh || true
        export PATH="$HOME/.local/bin:$PATH"
    fi
    
    # 4. Copy Configs & Blueprints (Templates)
    CURRENT_STEP="Assets Deployment"
    section "Deploying Configuration Templates & Assets"
    if [[ -d "config" ]]; then
        cp -r config/. "$KOAD_HOME/config/"
        ok "Configuration assets copied to $KOAD_HOME/config"
    fi

    # 4b. Infrastructure Boot (Docker with CASS setup)
    CURRENT_STEP="Infrastructure Boot"
    section "Infrastructure Boot (Docker with CASS)"
    info "Starting CASS, Redis, and Qdrant containers..."
    if command -v "docker-compose" &>/dev/null; then
        docker-compose up -d --build
    else
        docker compose up -d --build
    fi
    ok "gRPC and Docker infrastructure is online with CASS."
    
    # Deploy skills
    if [[ -d "plugin/skills" ]]; then
        cp -r plugin/skills/. "$KOAD_HOME/skills/"
        ok "Skills deployed to $KOAD_HOME/skills"
    fi
    
    # Deploy docker/rook assets
    if [[ -d "docker/rook" ]]; then
        cp -r docker/rook/. "$KOAD_HOME/docker/rook/"
        ok "Docker/Rook assets deployed to $KOAD_HOME/docker/rook"
    fi
    
    # Copy scripts
    if [[ -d "scripts" ]]; then
        mkdir -p "$KOAD_HOME/scripts"
        cp -r scripts/. "$KOAD_HOME/scripts/"
        ok "Scripts deployed to $KOAD_HOME/scripts"
    fi

    # 5. Environment & Configuration Hydration
    CURRENT_STEP="Config Hydration"
    section "Environment & Redis Configuration Setup"
    if [[ -f "$KOAD_HOME/.env" ]]; then
        ok ".env file already exists"
    else
        if [[ -f ".env.template" ]]; then
            cp .env.template "$KOAD_HOME/.env"
            portable_sed "s|KOADOS_HOME=.*|KOADOS_HOME=$KOAD_HOME|" "$KOAD_HOME/.env"
            ok ".env template initialized"
        fi
    fi
    
    if [[ ! -f "$KOAD_HOME/run/redis.active.conf" ]]; then
        local koad_home_escaped
        koad_home_escaped=$(echo "$KOAD_HOME" | sed 's/\//\\\//g')
        if [[ -f "$KOAD_HOME/config/defaults/redis.conf.template" ]]; then
            sed "s/{{KOAD_HOME}}/$koad_home_escaped/g" \
                "$KOAD_HOME/config/defaults/redis.conf.template" > "$KOAD_HOME/run/redis.active.conf"
            ok "redis.active.conf hydrated"
        fi
    fi
    
    # 6. Binary Compilation & Copy
    CURRENT_STEP="Binary Compilation"
    section "Compiling KoadOS Release Binaries"
    cargo build --release
    ok "Compilation complete."
    
    local bins=(koad koad-agent koad-cass koad-citadel koad-fs-mcp koad-map koad-notion-mcp koad-os-mcp invoke-tool register-tool cass-ingest koad-mcp)
    for bin in "${bins[@]}"; do
        if [[ -f "target/release/$bin" ]]; then
            rm -f "$BIN_DIR/$bin"
            cp "target/release/$bin" "$BIN_DIR/$bin"
            ok "$bin installed to $BIN_DIR"
        fi
    done
    
    # Shell helpers
    if [[ -f "scripts/koad-functions.sh" ]]; then
        cp "scripts/koad-functions.sh" "$BIN_DIR/koad-functions.sh"
    fi
    if [[ -f "plugin/bin/agent-boot.sh" ]]; then
        cp "plugin/bin/agent-boot.sh" "$BIN_DIR/agent-boot.sh"
    fi

    # 7. Systemd Service Deployment
    CURRENT_STEP="Systemd Service Deployment"
    section "Systemd Service Configuration"
    local setup_systemd=false
    if [[ -t 0 ]]; then
        read -p "Would you like to install systemd services for auto-start? [y/N]: " sys_choice
        if [[ "$sys_choice" =~ ^[Yy]$ ]]; then
            setup_systemd=true
        fi
    else
        setup_systemd=true
    fi
    
    if [[ "$setup_systemd" = true ]]; then
        info "Installing systemd unit files..."
        local service_src="$KOAD_HOME/config/systemd"
        local systemd_dir="/etc/systemd/system"
        for svc in koad-citadel koad-cass; do
            if [[ -f "$service_src/$svc.service.template" ]]; then
                sed "s|{{USER}}|$USER|g; s|{{KOAD_HOME}}|$KOAD_HOME|g" \
                    "$service_src/$svc.service.template" > "/tmp/$svc.service"
                sudo cp "/tmp/$svc.service" "$systemd_dir/$svc.service"
                rm -f "/tmp/$svc.service"
                sudo systemctl daemon-reload
                sudo systemctl enable "$svc.service"
                ok "Systemd unit $svc enabled"
            fi
        done
        info "Start services later with: sudo systemctl start koad-citadel koad-cass"
    fi

    # 8. Run Initialization Step
    CURRENT_STEP="Citadel Identity Initialization"
    section "Initializing Citadel Identity"
    if [[ -f "./koad-init.sh" ]]; then
        ./koad-init.sh "$KOAD_HOME" --name "$citadel_name" --captain "$captain_name"
    fi
    
    # 9. Shell Integration
    CURRENT_STEP="Shell Integration Setup"
    section "Shell Integration Setup"
    local bashrc="$HOME/.bashrc"
    if ! grep -q "koad-functions.sh" "$bashrc"; then
        echo -e "\n# KoadOS Environment" >> "$bashrc"
        echo "export KOADOS_HOME=\"$KOAD_HOME\"" >> "$bashrc"
        echo "export PATH=\"\$KOADOS_HOME/bin:\$PATH\"" >> "$bashrc"
        echo "source \$KOADOS_HOME/bin/koad-functions.sh" >> "$bashrc"
        ok "Added KoadOS configuration to $bashrc"
        info "Please run 'source ~/.bashrc' or restart your terminal to activate."
    fi
    
    CURRENT_STEP="Done"
    ok "KoadOS installation and fresh setup is complete!"
    echo -e "\n${BOLD}Pro-Tip: Managing Agent Identities${RESET}"
    echo "Identities are local and private to this Citadel instance (git-ignored)."
    echo "To create/register a new agent identity, run:"
    echo -e "  ${CYAN}koad agent new <NAME> --rank <RANK> --role <ROLE>${RESET}\n"
}

# -----------------------------------------------------------------------------
# Main CLI Driver
# -----------------------------------------------------------------------------
MODE=""
KOAD_HOME=""

while [[ $# -gt 0 ]]; do
  case $1 in
    --update)
      MODE="update"
      shift
      ;;
    --install)
      MODE="install"
      shift
      ;;
    --home)
      KOAD_HOME="$2"
      shift 2
      ;;
    -h|--help)
      echo "Usage: $0 [--install | --update] [--home PATH]"
      echo "  --install   Completely install and setup a fresh KoadOS Citadel (default)"
      echo "  --update    Locate and update any locally installed Citadels"
      echo "  --home      Set custom KoadOS installation directory (defaults to ~/.koad-os)"
      exit 0
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

echo -e "${BOLD}"
echo "  ██╗  ██╗ ██████╗  █████╗ ██████╗      ██████╗ ███████╗"
echo "  ██║ ██╔╝██╔═══██╗██╔══██╗██╔══██╗    ██╔═══██╗██╔════╝"
echo "  █████╔╝ ██║   ██║███████║██║  ██║    ██║   ██║███████╗"
echo "  ██╔═██╗ ██║   ██║██╔══██║██║  ██║    ██║   ██║╚════██║"
echo "  ██║  ██╗╚██████╔╝██║  ██║██████╔╝    ╚██████╔╝███████║"
echo "  ╚═╝  ╚═╝ ╚═════╝ ╚═╝  ╚═╝╚═════╝      ╚═════╝ ╚══════╝"
echo -e "${RESET}"
echo "  KoadOS Setup Manager  ·  v3.2.0"
echo

if [[ -z "$MODE" ]]; then
    if [[ -t 0 ]]; then
        echo -e "${BOLD}Select KoadOS Mode:${RESET}"
        echo "1) Update existing Citadel installations (Locate & Update)"
        echo "2) Install a fresh KoadOS Citadel"
        read -p "Select option [1-2]: " opt
        case $opt in
            1) MODE="update" ;;
            2) MODE="install" ;;
            *) echo "Invalid option"; exit 1 ;;
        esac
    else
        MODE="install"
    fi
fi

if [[ "$MODE" == "update" ]]; then
    run_update
else
    run_install
fi
