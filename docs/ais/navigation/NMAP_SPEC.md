# AIS nMap Spec (Normalized Map)

## Why nMap
`koad map` is runtime/dynamic. nMap is the normalized documentation schema that captures stable navigation surfaces so agents can quickly orient even before runtime probes.

## nMap Record Schema
Each record should include:
- alias: short navigation key (example: `citadel-root`)
- path: canonical absolute or repo-relative path
- community: functional grouping (kernel, signal, cognition, integration)
- purpose: one-line responsibility
- owner: responsible role/persona/team
- update_trigger: when this entry must be reviewed

## Required nMap Domains
1) Repo domains
- crates/
- docs/
- plugin/skills/
- blueprints/
- updates/

2) Runtime domains
- `~/.citadel-jupiter/config/`
- `~/.citadel-jupiter/docs/`
- `~/.citadel-jupiter/logs/`
- `~/.citadel-jupiter/data/db/`

## Output Modes
- Human mode: markdown table (default in docs)
- Machine mode: optional JSON export (future)

## Maintenance
- When adding a major directory, add/update an nMap record in `registry/FILESYSTEM_SURFACES.md`.
- During release prep, verify aliases still resolve.
