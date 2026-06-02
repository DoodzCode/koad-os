---
name: rtk
description: Use when checking token savings, discovering missed compression opportunities, or explicitly invoking RTK commands outside of the automatic hook rewriting.
---

# RTK — Rust Token Killer

Token compression proxy for CLI output. The PreToolUse hook rewrites Bash commands automatically — agents interact directly only for analytics and discovery.

## Hook Behavior (Automatic)

```
PreToolUse (Bash) → rtk hook claude rewrites command transparently
```

`git status` becomes `rtk git status`. Zero overhead. No action needed.

## Agent-Facing Commands (Use These Directly)

### Check savings
```bash
rtk gain                  # session + global token savings summary
rtk gain --history        # per-command breakdown
rtk cc-economics          # Claude Code spend vs RTK savings analysis
```

### Find missed opportunities
```bash
rtk discover              # scan recent Bash history for uncompressed commands
```

Run `rtk discover` if efficiency meter is low — shows exactly which commands to switch.

## Key Proxied Commands

| Raw | RTK equivalent | Typical savings |
|---|---|---|
| `git log` | `rtk git log` | ~2K tokens |
| `cat <file>` | `rtk read <file>` | ~6-16K tokens |
| `grep <pattern>` | `rtk grep <pattern>` | significant |
| `ls -la` | `rtk ls -la` | 60-75% |
| `git commit` | `rtk git commit` | 97% |
| `docker ps` | `rtk docker ps` | varies |
| `gh pr list` | `rtk gh pr list` | varies |

Hook handles these automatically. Table is for awareness, not manual use.

## When to Call RTK Directly

- `rtk gain` — to audit token efficiency at session end or on request
- `rtk discover` — after `rtk gain` shows low efficiency meter
- `rtk proxy <cmd>` — debug: run command without RTK filtering

## Common Mistakes

| Mistake | Fix |
|---|---|
| Manually prepending `rtk` to every command | Hook does it — don't double-wrap |
| Never checking `rtk gain` | Run periodically to verify hook is active |
| Ignoring low efficiency meter | Run `rtk discover` to find what's slipping through |
