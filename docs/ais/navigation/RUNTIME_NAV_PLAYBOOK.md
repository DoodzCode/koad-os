# AIS Runtime Navigation Playbook

## Session Start
Run after boot:
1. `koad map look`
2. `koad map nearby`
3. `koad map exits`

Publish brief orientation output in your worklog:
- current location
- nearby critical POIs
- reachable next paths

## Navigation Patterns
- Need target location quickly: `koad map goto <alias>`
- Need context around current work: `koad map nearby`
- Need to locate file/service: `koad map where <target>`
- Need backtracking: `koad map history`

## Escalation Path
If `koad map` context is incomplete:
1) check `docs/ais/registry/FILESYSTEM_SURFACES.md`
2) run focused file discovery
3) patch AIS registry after discovering missing stable surfaces

## Rule
Do not begin non-trivial implementation before orientation pass unless incident response requires immediate action.
