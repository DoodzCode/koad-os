#!/usr/bin/env bash
# =============================================================================
# KoadOS Initialization Script
# =============================================================================
# Prevent sourcing the script to avoid closing parent terminal on exits/traps
if [[ "${BASH_SOURCE[0]}" != "${0}" ]]; then
    echo -e "\033[0;31m✗\033[0m Error: This script must be executed directly, not sourced."
    echo "Run it as: ./koad-init.sh [KOAD_HOME]"
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

# Argument Parsing
FORCE=false
CUSTOM_NAME=""
CUSTOM_CAPTAIN=""
POSITIONAL_ARGS=()

while [[ $# -gt 0 ]]; do
  case $1 in
    --name)
      CUSTOM_NAME="$2"
      shift 2
      ;;
    --captain)
      CUSTOM_CAPTAIN="$2"
      shift 2
      ;;
    --force)
      FORCE=true
      shift
      ;;
    -h|--help)
      echo "Usage: $0 [KOAD_HOME] [--name NAME] [--captain CAPTAIN_NAME] [--force]"
      exit 0
      ;;
    *)
      POSITIONAL_ARGS+=("$1")
      shift
      ;;
  esac
done

KOAD_HOME="${KOADOS_HOME:-${POSITIONAL_ARGS[0]:-$HOME/.koad-os}}"
BIN_DIR="$KOAD_HOME/bin"

# Cleanup Logic
TMP_DIR=$(mktemp -d)
cleanup() {
    local exit_code=$?
    if [[ -d "$TMP_DIR" ]]; then
        rm -rf "$TMP_DIR"
    fi
    if [[ $exit_code -ne 0 ]]; then
        fail "Initialization failed at step: ${CURRENT_STEP:-Unknown}"
    fi
}
trap cleanup EXIT ERR INT TERM

# 0. Root Check & Portability Helpers
CURRENT_STEP="Root Check"
if [[ ! -d "blueprints" || ! -f "Cargo.toml" ]]; then
    fail "Execution error: Run this script from the root of the koad-os repository."
    exit 1
fi

portable_sed() {
    local pattern="$1"
    local file="$2"
    local tmpfile
    tmpfile=$(mktemp "$TMP_DIR/sed.XXXXXX")
    sed "$pattern" "$file" > "$tmpfile" && mv "$tmpfile" "$file"
}

section "KoadOS Initialization"

# 1. Citadel Identity
CURRENT_STEP="Citadel Identity"
echo -e "${BOLD}Step 1: Citadel Identity${RESET}"

if [[ -z "$CUSTOM_NAME" ]]; then
    if [[ -t 0 ]]; then
        read -p "Enter your Citadel Name [Sanctuary]: " CUSTOM_NAME
    fi
fi
CITADEL_NAME=${CUSTOM_NAME:-Sanctuary}
info "Initializing Citadel: $CITADEL_NAME"

if [[ -z "$CUSTOM_CAPTAIN" ]]; then
    if [[ -t 0 ]]; then
        read -p "Enter your Captain Agent Name [Tyr]: " CUSTOM_CAPTAIN
    fi
fi
CAPTAIN_NAME=${CUSTOM_CAPTAIN:-Tyr}
info "Initializing Captain Agent: $CAPTAIN_NAME"

# 2. Directory Setup
CURRENT_STEP="Directory Setup"
section "Directory Setup"
for dir in "$BIN_DIR" "$KOAD_HOME/agents/captain" "$KOAD_HOME/config" "$KOAD_HOME/cache" "$KOAD_HOME/data/db" "$KOAD_HOME/logs" "$KOAD_HOME/run"; do
    if [[ ! -d "$dir" ]]; then
        mkdir -p "$dir"
        ok "$dir created"
    else
        ok "$dir already exists"
    fi
done

# 3. .env Initialization
CURRENT_STEP="Environment Setup"
section "Environment Setup"
if [[ -f "$KOAD_HOME/.env" && "$FORCE" = false ]]; then
    ok ".env already exists (use --force to overwrite)"
else
    if [[ -f ".env.template" ]]; then
        cp .env.template "$KOAD_HOME/.env"
        # Attempt to set KOADOS_HOME in the new .env
        portable_sed "s|KOADOS_HOME=.*|KOADOS_HOME=$KOAD_HOME|" "$KOAD_HOME/.env"
        ok ".env initialized from template"
    else
        warn ".env.template not found. Skipping .env initialization."
    fi
fi

# 4. Captain Identity Initialization
CURRENT_STEP="Captain Identity Setup"
section "Captain Identity Setup"
if [[ -f "$KOAD_HOME/agents/captain/IDENTITY.toml" && "$FORCE" = false ]]; then
    ok "Captain identity already exists (use --force to overwrite)"
else
    if [[ -d "blueprints/captain" ]]; then
        cp blueprints/captain/IDENTITY.toml "$KOAD_HOME/agents/captain/IDENTITY.toml"
        cp blueprints/captain/SYSTEM.md "$KOAD_HOME/agents/captain/SYSTEM.md"

        # Customize IDENTITY.toml
        portable_sed "s/name = \"Captain\"/name = \"$CAPTAIN_NAME\"/" "$KOAD_HOME/agents/captain/IDENTITY.toml"
        portable_sed "s/station = \"Citadel\"/station = \"$CITADEL_NAME\"/" "$KOAD_HOME/agents/captain/IDENTITY.toml"
        ok "Captain identity initialized for $CITADEL_NAME ($CAPTAIN_NAME)"

        # Write private agent identity file (git-ignored)
        CAPTAIN_KEY=$(echo "$CAPTAIN_NAME" | tr '[:upper:]' '[:lower:]')
        cat <<EOF > "$KOAD_HOME/config/identities/${CAPTAIN_KEY}.toml"
[identities.${CAPTAIN_KEY}]
name = "${CAPTAIN_NAME}"
rank = "Captain"
role = "Admin and Principal Engineer"
bio = "Flagship KoadOS Agent. Principal Systems & Operations Engineer; Captain of the Koados ${CITADEL_NAME}. Optimized for deep memory, structural integrity, and station-wide orchestration."
runtime = "claude"
tier = 4
vault = "~/.${CAPTAIN_KEY}"
EOF
        ok "Private agent identity file created at config/identities/${CAPTAIN_KEY}.toml"

        # Generate root harness context files (gitignored, machine-specific)
        GENERATED_AT="$(date -u +%Y-%m-%dT%H:%M:%SZ)"
        for context_file in CLAUDE.md AGENTS.md GEMINI.md; do
            blueprint="blueprints/captain/$context_file"
            if [[ -f "$blueprint" ]]; then
                cp "$blueprint" "./$context_file"
                portable_sed "s|{{KOADOS_HOME}}|$KOAD_HOME|g" "./$context_file"
                portable_sed "s|{{CITADEL_NAME}}|$CITADEL_NAME|g" "./$context_file"
                portable_sed "s|{{GENERATED_AT}}|$GENERATED_AT|g" "./$context_file"
                ok "$context_file generated"
            fi
        done
    else
        fail "Blueprints not found. Captain identity could not be initialized."
    fi
fi

# 5. Shell Helper Installation
CURRENT_STEP="Shell Helpers"
section "Shell Helpers"
if [[ -f "scripts/koad-functions.sh" ]]; then
    cp scripts/koad-functions.sh "$BIN_DIR/koad-functions.sh"
    ok "koad-functions.sh installed to $BIN_DIR"
else
    warn "scripts/koad-functions.sh not found."
fi

if [[ -f "plugin/bin/agent-boot.sh" ]]; then
    cp plugin/bin/agent-boot.sh "$BIN_DIR/agent-boot.sh"
    ok "agent-boot.sh installed to $BIN_DIR"
else
    warn "plugin/bin/agent-boot.sh not found."
fi

# 5b. Skills Deployment
CURRENT_STEP="Skills Deployment"
section "Skills Deployment"
if [[ -d "plugin/skills" ]]; then
    mkdir -p "$KOAD_HOME/skills"
    cp -r plugin/skills/. "$KOAD_HOME/skills/"
    ok "Skills deployed to $KOAD_HOME/skills"
else
    warn "plugin/skills/ not found. Skipping."
fi

# 6. Binary Installation (Final Check)
CURRENT_STEP="Binary Installation"
section "Binary Installation"
for bin in koad koad-agent koad-cass koad-citadel koad-os-mcp; do
    if [[ -f "target/release/$bin" ]]; then
        # Remove symlink or old binary before copy
        rm -f "$BIN_DIR/$bin"
        cp "target/release/$bin" "$BIN_DIR/$bin"
        ok "$bin installed to $BIN_DIR"
    else
        warn "$bin not found in target/release/. Ensure you ran ./install.sh first."
    fi
done

# 6b. Docker Rook Assets
CURRENT_STEP="Docker Rook Deploy"
section "Docker Rook Assets"
ROOK_DEST="$KOAD_HOME/docker/rook"
mkdir -p "$ROOK_DEST"
cp -r docker/rook/. "$ROOK_DEST/"
ok "docker/rook/ deployed to $ROOK_DEST"

# 7. Database Migrations / Setup
CURRENT_STEP="Database & State"
section "Database & State"
# Future: Run migrations here
ok "Database state verified."

# 8. Graph Initialization
CURRENT_STEP="Graph Initialization"
section "Graph-Centric Navigation Setup"
if command -v "code-review-graph" &>/dev/null; then
    info "Initializing and building the codebase graph..."
    code-review-graph init
    code-review-graph build
    ok "Graph built successfully."
else
    warn "code-review-graph not found. Graph-centric navigation will be disabled."
    info "To enable it, install the tool: ${BOLD}pipx install code-review-graph${RESET}"
    info "Then run: ${CYAN}code-review-graph init && code-review-graph build${RESET}"
fi

# 9. Claude Desktop Memory Agent (Optional)
CURRENT_STEP="Claude Desktop Agent Setup"
section "Claude Desktop Memory Agent (Optional)"
info "KoadOS can run a local memory bridge for Claude Desktop."
info "This gives your Claude Desktop agent persistent memory via CASS."

SETUP_CLAUDE_AGENT=false
if [[ -t 0 ]]; then
    read -p "  Set up a Claude Desktop memory agent? [y/N]: " setup_agent_input
    if [[ "$setup_agent_input" =~ ^[Yy]$ ]]; then
        SETUP_CLAUDE_AGENT=true
    fi
fi

if [[ "$SETUP_CLAUDE_AGENT" = true ]]; then
    CLAUDE_AGENT_NAME=""
    if [[ -t 0 ]]; then
        read -p "  Enter a name for your Claude Desktop agent [Scout]: " CLAUDE_AGENT_NAME
    fi
    CLAUDE_AGENT_NAME="${CLAUDE_AGENT_NAME:-Scout}"

    # Store in .env
    if [[ -f "$KOAD_HOME/.env" ]]; then
        if grep -q "KOADOS_CLAUDE_AGENT_NAME" "$KOAD_HOME/.env"; then
            portable_sed "s|KOADOS_CLAUDE_AGENT_NAME=.*|KOADOS_CLAUDE_AGENT_NAME=$CLAUDE_AGENT_NAME|" "$KOAD_HOME/.env"
        else
            echo "KOADOS_CLAUDE_AGENT_NAME=$CLAUDE_AGENT_NAME" >> "$KOAD_HOME/.env"
        fi
        ok "Claude Desktop agent name '$CLAUDE_AGENT_NAME' saved to .env"
    fi

    echo ""
    echo -e "${BOLD}To start your Claude Desktop memory agent:${RESET}"
    echo -e "  ${CYAN}AGENT_NAME=\"$CLAUDE_AGENT_NAME\" $KOAD_HOME/docker/rook/rook-up.sh${RESET}"
    echo ""
    echo -e "Then add this to your ${BOLD}claude_desktop_config.json${RESET}:"
    echo -e "  ${CYAN}\"mcpServers\": { \"$CLAUDE_AGENT_NAME\": { \"transport\": \"http\", \"url\": \"http://localhost:9742/mcp\" } }${RESET}"
    echo ""
    ok "Claude Desktop agent '$CLAUDE_AGENT_NAME' configured."
else
    info "Skipping Claude Desktop agent setup. Run '$KOAD_HOME/docker/rook/rook-up.sh' later to enable."
fi

CURRENT_STEP="Finalizing"
section "Initialization Complete"
info "Citadel '$CITADEL_NAME' is ready."
echo -e "\n${BOLD}Final Actions:${RESET}"
echo -e "1. Add KoadOS to your shell (e.g., ~/.bashrc):"
echo -e "   ${CYAN}export KOADOS_HOME=\"$KOAD_HOME\"${RESET}"
echo -e "   ${CYAN}export PATH=\"\$KOADOS_HOME/bin:\$PATH\"${RESET}"
echo -e "   ${CYAN}source \$KOADOS_HOME/bin/koad-functions.sh${RESET}"
echo -e "2. Reload your shell: ${CYAN}source ~/.bashrc${RESET}"
echo -e "3. Boot your Captain: ${BOLD}agent-boot captain${RESET}\n"
