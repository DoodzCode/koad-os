# AIS (Agent Information System)

Purpose: AIS is the Citadel agent operating system documentation layer. It defines how agents orient, navigate, recall knowledge, track progress, and execute with governance.

## Scope
- Rank scope: all Citadel agents; Officer+ doctrines are mandatory where specified.
- Runtime scope: source canon in `koados-citadel`, operational mirror in `~/.citadel-jupiter/docs/ais/`.

## AIS Structure
- `architecture/AGENT_OPERATING_SYSTEM.md` — AIS architecture and operating model
- `navigation/NMAP_SPEC.md` — normalized map (nMap) spec for filesystem/system navigation
- `navigation/RUNTIME_NAV_PLAYBOOK.md` — runtime navigation workflow (`koad map`, look/nearby/exits)
- `operations/KNOWLEDGE_LIFECYCLE.md` — knowledge freshness, memory and doc update rules
- `operations/PROGRESS_TRACKING.md` — development progress, status surfaces, and reporting cadence
- `registry/FILESYSTEM_SURFACES.md` — canonical path registry and ownership map
- `governance/AIS_MAINTENANCE_CHARTER.md` — builder/maintainer contract and review cadence
- `protocols/` — enforceable AIS doctrine documents

## Required Session Bootstrap (Officer+)
1) Run agent boot sequence and health checks.
2) Run navigation triad:
   - `koad map look`
   - `koad map nearby`
   - `koad map exits`
3) Load active AIS protocols from `protocols/`.
4) For non-trivial work, execute SEG gate before committing to execution.

## Definition of Done for AIS updates
- Canon docs updated in this repo.
- Cross-links remain valid and discoverable from this file.
- Operational mirror synced to `~/.citadel-jupiter/docs/ais/`.
- Update note added under `updates/` when behavior/contract changes.
