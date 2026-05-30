# AIS Progress Tracking Standard

## Goal
Keep development progress visible and machine-locatable for all agents.

## Required Surfaces
- `SITREP.md` — active objective and short mission board
- `updates/*.md` — timestamped change records
- `docs/incidents/*.md` — incident root cause and remediation records

## SITREP Contract
SITREP should include:
1) active objective
2) top active missions (checkbox list)
3) recent accomplishments
4) architectural decisions
5) next actions

## Cadence
- Update SITREP when objective changes or at least weekly.
- Add update note for any AIS doctrine/contract changes.

## AIS Backlog (recommended)
Track AIS-specific gaps in a dedicated section of SITREP:
- missing docs
- stale docs
- ownership gaps
- navigation blind spots
