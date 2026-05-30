# AIS Architecture: Agent Operating System

## Mission
Provide a unified operating model for all Citadel agents so they can:
- understand where they are
- find what matters quickly
- act with current knowledge
- publish progress in a consistent format

## Core Planes
1. Orientation Plane
   - identity, rank, role, runtime health, mission objective
   - entrypoints: `agent-boot`, `koad status`, AIS protocols

2. Navigation Plane
   - workspace and system topology
   - entrypoints: `koad map` + nMap registry

3. Knowledge Plane
   - durable facts, learnings, and doctrine
   - entrypoints: CASS, AIS docs, protocol docs

4. Execution Plane
   - accepted work contracts, safeguards, and progress reporting
   - entrypoints: SEG doctrine, SITREP/progress docs, update logs

## Contract
All agent-facing operational docs should map to one AIS plane.
If a doc has no plane, refactor it or archive it.

## Anti-Drift Rules
- Single source of truth for doctrine: `docs/ais/protocols/`.
- System path truths must exist in `registry/FILESYSTEM_SURFACES.md`.
- Stale mission plans older than current architecture should move to `docs/inbox/` or `docs/incidents/`.
