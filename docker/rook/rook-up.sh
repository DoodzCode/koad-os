#!/usr/bin/env bash
# rook-up.sh — Bootstrap Claude Desktop CASS Memory Bridge
# Run once per machine to stand up the local memory stack.
# Usage: AGENT_NAME="Scout" ./rook-up.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
DATA_DIR="$SCRIPT_DIR/data"

# Load agent name: env var > KOAD .env > default
if [[ -z "${AGENT_NAME:-}" ]]; then
    KOAD_ENV="${KOADOS_HOME:-$HOME/.koad-os}/.env"
    if [[ -f "$KOAD_ENV" ]]; then
        AGENT_NAME=$(grep "^KOADOS_CLAUDE_AGENT_NAME=" "$KOAD_ENV" | cut -d= -f2)
    fi
fi
AGENT_NAME="${AGENT_NAME:-agent}"
export AGENT_NAME

echo "[$AGENT_NAME] Standing up CASS memory stack..."

# Create data directories
mkdir -p "$DATA_DIR/redis" "$DATA_DIR/qdrant" "$DATA_DIR/sqlite"

# Pull images
echo "[$AGENT_NAME] Pulling images..."
docker compose -f "$SCRIPT_DIR/docker-compose.yml" pull

# Start stack
echo "[$AGENT_NAME] Starting services..."
docker compose -f "$SCRIPT_DIR/docker-compose.yml" up -d

# Wait for MCP to be ready
echo "[$AGENT_NAME] Waiting for MCP server..."
for i in $(seq 1 20); do
    if curl -sf http://localhost:9742/health >/dev/null 2>&1; then
        echo "[$AGENT_NAME] MCP server ready."
        break
    fi
    sleep 2
    if [ "$i" -eq 20 ]; then
        echo "[$AGENT_NAME] WARNING: MCP server did not respond after 40s. Check: docker compose -f $SCRIPT_DIR/docker-compose.yml logs koad-os-mcp"
    fi
done

echo ""
echo "┌──────────────────────────────────────────────────────────────────┐"
echo "│  $AGENT_NAME is online. Add this to claude_desktop_config.json: │"
echo "│                                                                  │"
echo "│  \"mcpServers\": {                                                │"
echo "│    \"$AGENT_NAME\": {                                             │"
echo "│      \"transport\": \"http\",                                       │"
echo "│      \"url\": \"http://localhost:9742/mcp\"                         │"
echo "│    }                                                             │"
echo "│  }                                                               │"
echo "└──────────────────────────────────────────────────────────────────┘"
