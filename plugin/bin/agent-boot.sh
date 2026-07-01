#!/usr/bin/env bash
# agent-boot.sh — Canonical KoadOS agent boot shell wrapper.
# Sourced from within agent-boot() function in koad-functions.sh.
# Delegates all environment and workspace hydration to the compiled koad-agent Rust command.

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
        echo "[agent-boot] ERROR: KOAD_AGENT_NAME is not set. Please run agent-prep <agent-name> first." >&2
        return 1
    fi

    local _target_home="${KOAD_HOME:-$KOADOS_HOME}"
    _target_home="${_target_home:-$HOME/.koad-os}"

    if [ ! -f "$_target_home/bin/koad-agent" ]; then
        echo "[agent-boot] ERROR: koad-agent binary not found at $_target_home/bin/koad-agent" >&2
        return 1
    fi

    eval "$("$_target_home/bin/koad-agent" boot "$agent_name")"
}

_koad_agent_boot "$@"
