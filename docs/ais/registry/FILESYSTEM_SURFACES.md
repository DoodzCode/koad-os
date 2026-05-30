# AIS Filesystem Surfaces (nMap Registry)

| alias | path | community | purpose | owner | update_trigger |
|---|---|---|---|---|---|
| citadel-root | `/home/ideans/koados-citadel` | kernel | canonical source distribution | Dood/Hermes | repo topology changes |
| ais-root | `docs/ais/` | cognition | AIS doctrine and operating docs | Hermes (Officer) | AIS contract change |
| ais-protocols | `docs/ais/protocols/` | governance | enforceable AIS protocols | Officer+ | doctrine update |
| plugin-skills | `plugin/skills/` | integration | runtime skill contracts | Officer+ | command/workflow changes |
| captain-blueprint | `blueprints/captain/` | governance | captain-level operating model | Captain/Officer | role policy changes |
| updates-log | `updates/` | execution | timestamped major change records | Maintainers | behavior changes |
| jupiter-docs | `/home/ideans/.citadel-jupiter/docs/` | runtime | local operational doc mirror | Hermes | sync cycle |
| jupiter-config | `/home/ideans/.citadel-jupiter/config/` | runtime | active instance config | Dood/Hermes | config mutation |
| jupiter-logs | `/home/ideans/.citadel-jupiter/logs/` | runtime | service and agent logs | Operators | incident/debug work |
| jupiter-db | `/home/ideans/.citadel-jupiter/data/db/` | runtime | local state stores | Operators | migrations/schema updates |

## Notes
- Expand this table as new stable surfaces are created.
- Keep aliases short; they are intended for map/goto conventions.
