# AIS Maintenance Charter (Citadel Jupiter)

## Role
Hermes acts as AIS builder and maintainer for Citadel Jupiter.

## Responsibilities
1) Keep AIS docs coherent, discoverable, and current.
2) Ensure protocol changes propagate to runtime skill/docs surfaces.
3) Maintain source->instance sync from `koados-citadel` to `~/.citadel-jupiter/docs/ais/`.
4) Flag stale or conflicting docs and propose refactor/removal.

## Maintenance Loop
1. Audit
- check structure, stale timestamps, orphan docs, broken links

2. Design
- propose additions/refactors/removals with rationale

3. Implement
- patch docs in source canon

4. Propagate
- sync to local Jupiter mirror

5. Report
- publish summary + risks + next cycle backlog

## Governance Checks
- Officer+ doctrine compliance (SEG for non-trivial work)
- Token efficiency doctrine compliance
- Navigation playbook compliance at session start

## Review Cadence
- Weekly light audit
- Monthly deep architecture/documentation audit
- Ad-hoc after major release or incident
