#!/bin/bash
# KoadOS Jupiter — Install systemd service units
# Run once after migration to enable auto-start of Citadel services

set -e

KOAD_HOME="${KOADOS_HOME:-${SUDO_HOME:+/home/$SUDO_USER}/.koad-os}"
KOAD_HOME="${KOAD_HOME:-$HOME/.koad-os}"
SYSTEMD_DIR="/etc/systemd/system"
SERVICE_SRC="$KOAD_HOME/config/systemd"

echo "[KoadOS] Installing systemd service units..."

for svc in koad-citadel koad-cass; do
    echo "  → Installing $svc.service"
    sudo cp "$SERVICE_SRC/$svc.service" "$SYSTEMD_DIR/$svc.service"
    sudo systemctl daemon-reload
    sudo systemctl enable "$svc.service"
    echo "  ✓ $svc enabled"
done

echo "[KoadOS] Services installed. Start with: sudo systemctl start koad-citadel koad-cass"
echo "[KoadOS] Status: systemctl status koad-citadel koad-cass"
