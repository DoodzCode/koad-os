# Citadel SITREP (Situation Report)
**Date:** 2026-05-31
**Current Objective:** KoadOS repository synchronization, packaging, and robust installation/update mechanics.

## 🎯 Active Missions
- [ ] **Fleet Distribution Prep:** Verify fresh installation on external systems and prepare release tag.
- [ ] **AIS Documentation Sync:** Complete operating documents for the unified installer and `--update` flags.
- [ ] **Docker Integration Guide:** Document WSL Resource configuration constraints for new developers.

## 🗂 AIS Backlog
- [ ] Add machine-readable nMap JSON export workflow.
- [ ] Add link-check script for AIS docs and protocol references.
- [ ] Add owner metadata and last-reviewed dates to legacy docs.

## 🛠️ Recent Accomplishments
- **CASS Token-Aware Memory Metadata (P2):** Added an optional, backward-compatible `MemoryMetadata` layer to CASS memories (token estimates, prompt-budget hints, retrieval/provenance/privacy) persisted in SQLite L2 + carried through L1 Redis, auto-populated at the gRPC commit choke point, surfaced via `memory.commit`/recall/search, and consumed by hydration for budget-aware, cache-stable prompt packing. Includes an offline `backfill_metadata` binary. See `docs/cass-memory-metadata.md`.
- **Unified Installer & Updater (P1):** Created a single root entrypoint `install.sh` supporting both `--install` (clean setups) and `--update` (automated system-wide upgrades).
- **Private Identity Isolation (P2):** Hardened `.gitignore` and cached indexes to ensure private Citadel metadata (`config/identities/*.toml`) is never leaked via git.
- **Shell-Crash Protection (P1):** Restructured shell script headers (`install.sh`, `koad-init.sh`, `koad-setup.sh`, `scripts/uninstall.sh`) so that sourcing checks occur before `set -euo pipefail`, preventing interactive shell terminations.
- **Docker/CASS Build Stabilization (P1):** Upgraded container builder environments to Rust `1.90` and mapped the workspace `proto/` definitions directory into the builder stages, resolving the `time-core` Edition 2024 compiler panic.

## 🏗️ Architectural Decisions
- **Private Identity Separation:** User agent settings must remain strictly decoupled from the core codebase. Identity profiles are created during initialization.
- **Rust Toolchain Modernization:** Container builder environments will track modern compiler versions (>=1.90) to prevent breakages due to upstream dependencies transitioning to Rust Edition 2024.

## 🔜 Immediate Next Actions
1. Push local changes to the remote branch (`git push`).
2. Verify native Docker WSL integration on the target system.
3. Test clean installation flow in a secondary sandbox container.
