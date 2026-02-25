#!/bin/bash

# KoadOS Automated Installer
# Designed to be executed by an AI Agent during the "First Contact" phase.

set -e

usage() {
    echo "Usage: $0 --partner <name> --persona <name> --role <role> --langs <lang1,lang2>"
    exit 1
}

# Parse Arguments
NON_INTERACTIVE=false
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --partner) PARTNER="$2"; shift ;;
        --persona) PERSONA="$2"; shift ;;
        --role) ROLE="$2"; shift ;;
        --langs) LANGS="$2"; shift ;;
        --non-interactive) NON_INTERACTIVE=true ;;
        *) usage ;;
    esac
    shift
done

if [[ -z "$PARTNER" || -z "$PERSONA" || -z "$ROLE" || -z "$LANGS" ]]; then
    usage
fi

echo "--- KoadOS Installation: Initializing Persona '$PERSONA' ---"

# 1. Pre-installation Checks
echo "Running system pre-checks..."

check_tool() {
    if command -v "$1" &> /dev/null; then
        version=$($1 --version 2>&1 | head -n 1)
        echo "[PASS] $1 found: $version"
        return 0
    else
        echo "[FAIL] $1 is missing."
        return 1
    fi
}

ERRORS=0
check_tool git || ERRORS=$((ERRORS+1))
check_tool cargo || ERRORS=$((ERRORS+1))
check_tool python3 || ERRORS=$((ERRORS+1))
check_tool node || echo "[WARN] Node.js is missing. Some skills may not function."
check_tool npm || echo "[WARN] NPM is missing. Some skills may not function."

if [ $ERRORS -gt 0 ]; then
    echo "Error: Essential tools (git, cargo, python3) are missing. Please install them and try again."
    exit 1
fi

# 2. Optional Features Consent
INSTALL_BOOSTER=false
if [ "$NON_INTERACTIVE" = false ]; then
    read -p "Install Cognitive Booster Daemon (background file tracking)? [y/N]: " booster_choice
    if [[ "$booster_choice" =~ ^[Yy]$ ]]; then
        INSTALL_BOOSTER=true
    fi
    
    if ! git config user.name &> /dev/null || ! git config user.email &> /dev/null; then
        echo "Git identity not detected."
        read -p "Configure basic Git identity for 'koad publish'? [y/N]: " git_choice
        if [[ "$git_choice" =~ ^[Yy]$ ]]; then
            read -p "Enter Git Name: " git_name
            read -p "Enter Git Email: " git_email
            git config --global user.name "$git_name"
            git config --global user.email "$git_email"
        fi
    fi
fi

# 3. Compile Core
echo "Compiling Rust core..."
(cd core/rust && cargo build --release)

# 4. Scaffold Operational Directories
echo "Scaffolding directories..."
mkdir -p bin skills/global drivers/gemini templates

# 5. Deploy Binaries
echo "Deploying binary..."
cp core/rust/target/release/koad bin/koad

if [ "$INSTALL_BOOSTER" = true ]; then
    echo "Deploying Cognitive Booster..."
    cp core/rust/target/release/koad-daemon bin/koad-daemon
fi

# 6. Generate Initial koad.json
if [ ! -f "koad.json" ]; then
    echo "Generating koad.json..."
    
    # Convert comma-separated langs to JSON array
    IFS=',' read -ra ADDR <<< "$LANGS"
    LANG_JSON=""
    for i in "${ADDR[@]}"; do
        LANG_JSON+="\"$i\","
    done
    LANG_JSON=${LANG_JSON%?} # Remove trailing comma

    cat <<EOF > koad.json
{
  "version": "2.4",
  "identity": {
    "name": "$PERSONA",
    "role": "$ROLE",
    "bio": "AI Software Engineering Partner optimized for $PARTNER."
  },
  "preferences": {
    "languages": [$LANG_JSON],
    "booster_enabled": $INSTALL_BOOSTER,
    "style": "programmatic-first",
    "principles": ["Simplicity first", "Plan before build"]
  },
  "drivers": {
    "gemini": {
      "bootstrap": "~/.koad-os/drivers/gemini/BOOT.md",
      "mcp_enabled": true,
      "tools": ["save_memory", "google_web_search", "run_shell_command"]
    }
  }
}
EOF
else
    echo "koad.json already exists. Skipping generation."
fi

# 7. Initialize Database
echo "Initializing SQLite database..."
./bin/koad init --force

# 8. Final Configuration Instructions
echo "--- Installation Complete ---"
echo "Partner: $PARTNER"
echo "Persona: $PERSONA"
echo ""
echo "CRITICAL: The following environment variables are required for full functionality."
echo "Please add them to your .bashrc, .zshrc, or .env file:"
echo ""
echo "export GITHUB_PERSONAL_PAT='your_github_pat'"
echo "export NOTION_TOKEN='your_notion_token'"
echo "export KOAD_HOME=\"\$HOME/.koad-os\""
echo "export PATH=\"\$KOAD_HOME/bin:\$PATH\""
echo ""
echo "NEXT STEPS:"
echo "1. Source your profile: 'source ~/.bashrc'"
echo "2. Run 'koad boot' to awaken the persona."
echo "3. Use 'koad sync notion --db-id <ID>' to initialize your Notion index."
