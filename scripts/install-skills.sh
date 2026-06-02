#!/usr/bin/env bash
# scripts/install-skills.sh — Installs repository-packaged Citadel agent skills globally for all agents on the system.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"

echo "=== Installing Citadel Agent Skills globally for all agents ==="

SKILLS=(
  "agent-boot"
  "tyr-boot"
  "koad-system"
  "koad-intel"
  "koad-map"
  "koad-signal"
  "koad-cognitive"
  "koad-fleet"
  "cass-recall"
  "cass-search"
  "code-review-graph"
  "rtk"
)

for skill in "${SKILLS[@]}"; do
  SKILL_PATH="$REPO_DIR/skills/$skill"
  if [ -d "$SKILL_PATH" ]; then
    echo "Installing skill: $skill..."
    npx skills add "$SKILL_PATH" -g -y -a '*'
  else
    echo "Warning: Skill directory $skill not found in repository, skipping."
  fi
done

echo "=== Citadel Agent Skills Installation Complete! ==="
