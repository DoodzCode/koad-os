#!/bin/bash
# KoadOS Shell Functions
# Source this file from your shell config (e.g., ~/.bashrc):
#   source $KOADOS_HOME/bin/koad-functions.sh

export KOAD_HOME="${KOADOS_HOME:-$HOME/.koad-os}"
export KOAD_BIN="$KOAD_HOME/bin"

# Auto-detect active runtime for agent body authorization.
# Override manually: export KOAD_RUNTIME=<runtime> before calling agent-boot.
if [ -z "$KOAD_RUNTIME" ]; then
    # Claude Code CLI sets CLAUDE_CODE_ENTRYPOINT in the subprocess environment
    if [ -n "$CLAUDE_CODE_ENTRYPOINT" ]; then
        export KOAD_RUNTIME="claude"
    # Gemini CLI (Node.js) — check process ancestry for known runtime signals
    elif [ -n "$GEMINI_API_KEY" ] || [ -n "$GOOGLE_GEMINI_API_KEY" ]; then
        export KOAD_RUNTIME="gemini"
    fi
fi

# agent-boot <name> [args]
# Boots an agent by hydrating the current shell with its identity and environment.
# Must be called as a shell function (not a subprocess) to propagate env vars.
# Boot logic is canonical in plugin/bin/agent-boot.sh — do not add logic here.
function agent-boot() {
    source "$KOAD_HOME/bin/agent-boot.sh" "$@"
}
export -f agent-boot

# agent-prep <name>
# Preps the current interactive shell as a body for the named KoadOS agent.
# Must be called as a shell function to propagate env vars.
function agent-prep() {
    local AGENT="${1:-}"
    if [ -z "$AGENT" ]; then
        echo "Usage: agent-prep <agent-name>" >&2
        return 1
    fi
    local KOAD_CMD="koad"
    if [ -f "$KOAD_BIN/koad" ]; then
        KOAD_CMD="$KOAD_BIN/koad"
    fi
    eval "$($KOAD_CMD agent prep "$AGENT")"
}
export -f agent-prep

# Alias --agentprep for convenience and backward compatibility
alias -- --agentprep='agent-prep'

