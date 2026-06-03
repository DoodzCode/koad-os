#!/usr/bin/env bash
# agent-boot.sh — Canonical KoadOS agent boot logic.
# MUST be sourced from within a shell function, never executed directly.
# Called by the agent-boot() wrapper in koad-functions.sh.
#
# Level flags (--quick, --full) are instruction-only: they are parsed by the
# agent skill layer (SKILL.md), not here. This script always performs the same
# shell-level boot (env hydration). Orientation behavior is determined by which
# skill level file the agent follows after boot completes.

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
    echo "[agent-boot] ERROR: This script must be sourced, not executed directly." >&2
    echo "[agent-boot] Usage: source agent-boot.sh <agent-name>" >&2
    exit 1
fi

_koad_agent_boot() {
    local agent_name="$1"
    if [ -z "$agent_name" ]; then
        agent_name="$KOAD_AGENT_NAME"
    fi
    if [ -z "$agent_name" ]; then
        echo "[agent-boot] Usage: agent-boot <agent-name>"
        return 1
    fi

    local _AGENT_LOWER
    _AGENT_LOWER=$(echo "$agent_name" | tr '[:upper:]' '[:lower:]')
    local _KOAD_HOME="$KOAD_HOME"
    local _AGENT_TOML="$_KOAD_HOME/config/identities/${_AGENT_LOWER}.toml"
    local _BRIEF_CACHE="$_KOAD_HOME/cache/session-brief-${_AGENT_LOWER}.md"

    # 1. Fast Display: Show the last known state immediately
    if [ -f "$_BRIEF_CACHE" ]; then
        echo -e "\x1b[1;30m[QUICK-RESTORE] Loading last cached brief...\x1b[0m"
        cat "$_BRIEF_CACHE"
        echo -e "\x1b[1;30m-------------------------------------------\x1b[0m"
    fi

    # 2. Runtime Detection: env signals take priority over TOML config.
    # koad-functions.sh runs a first-pass detection (env signals only) at source
    # time. This block is the TOML-fallback extension — it only fires when neither
    # env signal was present and KOAD_RUNTIME is still unset after that first pass.
    if [ -z "$KOAD_RUNTIME" ]; then
        if [ -n "$CLAUDE_CODE_ENTRYPOINT" ]; then
            export KOAD_RUNTIME="claude"
        elif [ -n "$GEMINI_API_KEY" ] || [ -n "$GOOGLE_GEMINI_API_KEY" ]; then
            export KOAD_RUNTIME="gemini"
        elif [ -f "$_AGENT_TOML" ]; then
            local _rt
            _rt=$(grep -E "^runtime[[:space:]]*=" "$_AGENT_TOML" | head -n1 | cut -d'"' -f2)
            [ -n "$_rt" ] && export KOAD_RUNTIME="$_rt"
        fi
    fi

    # 3. WSL GPU/CUDA path fix
    if [ -d "/usr/lib/wsl/lib" ]; then
        if [[ ":$LD_LIBRARY_PATH:" != *":/usr/lib/wsl/lib:"* ]]; then
            export LD_LIBRARY_PATH="/usr/lib/wsl/lib${LD_LIBRARY_PATH:+:${LD_LIBRARY_PATH}}"
        fi
    fi

    # 4. Async Hydration: eval koad-agent boot output to propagate env vars
    eval "$("$_KOAD_HOME/bin/koad-agent" boot "$agent_name")"
}

_koad_agent_boot "$@"
