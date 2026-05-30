# AIS Knowledge Lifecycle

## Knowledge Classes
1) Doctrine
- mandatory policy and behavior contracts
- location: `docs/ais/protocols/`

2) Operational Reference
- stable workflows and architecture truths
- location: `docs/ais/architecture/`, `docs/ais/navigation/`, `docs/ais/registry/`

3) Time-Bound State
- current mission or incident state
- location: `SITREP.md`, `docs/incidents/`, `updates/`

## Freshness Rules
- Doctrine: review monthly or on governance change.
- Operational reference: review on structural/codebase changes.
- Time-bound state: review weekly minimum.

## Drift Controls
- If command contracts change, patch corresponding AIS docs in same change set.
- If a path/ownership changes, patch `FILESYSTEM_SURFACES.md`.
- If a protocol changes behavior, create an update note under `updates/`.

## Memory + AIS Interlock
- CASS memory stores durable facts and learnings.
- AIS docs store canonical shared operating contracts.
- If a fact affects all agents, elevate from memory to AIS docs.
