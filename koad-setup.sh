#!/bin/bash

# KoadOS Automated Installer
# Designed to be executed by an AI Agent during the "First Contact" phase.

set -e

usage() {
    echo "Usage: $0 --partner <name> --persona <name> --role <role> --langs <lang1,lang2>"
    exit 1
}

# Parse Arguments
while [[ "$#" -gt 0 ]]; do
    case $1 in
        --partner) PARTNER="$2"; shift ;;
        --persona) PERSONA="$2"; shift ;;
        --role) ROLE="$2"; shift ;;
        --langs) LANGS="$2"; shift ;;
        *) usage ;;
    esac
    shift
done

if [[ -z "$PARTNER" || -z "$PERSONA" || -z "$ROLE" || -z "$LANGS" ]]; then
    usage
fi

echo "--- KoadOS Installation: Initializing Persona '$PERSONA' ---"

# 1. Dependency Check
if ! command -v cargo &> /dev/null; then
    echo "Error: Rust/Cargo is required for compilation."
    exit 1
fi

# 2. Compile Core
echo "Compiling Rust core..."
(cd core/rust && cargo build --release)

# 3. Scaffold Operational Directories
echo "Scaffolding directories..."
mkdir -p bin skills/global drivers/gemini templates

# 4. Deploy Binary
echo "Deploying binary..."
cp core/rust/target/release/koad bin/koad

# 5. Generate Initial koad.json
if [ ! -f "koad.json" ]; then
    echo "Generating koad.json..."
    
    # Convert comma-separated langs to JSON array
    IFS=',' read -ra ADDR <<< "$LANGS"
    LANG_JSON=""
    for i in "${ADDR[@]}"; do
        LANG_JSON+=""$i","
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

# 6. Initialize Database
echo "Initializing SQLite database..."
./bin/koad init

# 7. Final Configuration Instructions
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
