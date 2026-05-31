#!/usr/bin/env bash
# verify-services.sh — Pre-flight Qdrant readiness check for koad-cass.service
# Exits 0 when Qdrant is healthy on :6333, exits 1 on timeout.

TIMEOUT=30
INTERVAL=2
ELAPSED=0

echo "[CASS-PRE] Waiting for Qdrant on :6333..."

# If Qdrant container exists but is stopped, try to start it (best-effort).
if command -v docker &>/dev/null; then
    if docker ps -a --format '{{.Names}}' 2>/dev/null | grep -q '^qdrant$'; then
        if ! docker ps --format '{{.Names}}' 2>/dev/null | grep -q '^qdrant$'; then
            echo "[CASS-PRE] Qdrant container found but stopped — attempting docker start qdrant..."
            docker start qdrant &>/dev/null || true
        fi
    fi
fi

# Poll until healthy or timeout.
while [ "$ELAPSED" -lt "$TIMEOUT" ]; do
    if curl -sf http://localhost:6333/ &>/dev/null; then
        echo "[CASS-PRE] Qdrant ready."
        exit 0
    fi
    sleep "$INTERVAL"
    ELAPSED=$(( ELAPSED + INTERVAL ))
done

echo "[CASS-PRE] ERROR: Qdrant not ready after ${TIMEOUT}s."
exit 1
