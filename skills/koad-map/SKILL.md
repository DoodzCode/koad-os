---
name: koad-map
description: Use when orienting to a new directory, navigating the Citadel workspace, finding related configs or tasks nearby, or fast-traveling to a pinned location.
---

# koad map

Navigation HUD for KoadOS agents. Use at session start and whenever context or location shifts.

## Core Commands

| Command | Purpose |
|---|---|
| `koad map look` | Describe current directory, community, and nearby POIs |
| `koad map exits` | Show parent, siblings, and connected paths |
| `koad map nearby` | Surface contextually relevant tasks, KAPVs, and configs |
| `koad map goto <alias>` | Fast-travel to a pinned location |
| `koad map pins` | List all bookmarked locations |
| `koad map pin` | Bookmark current location |
| `koad map where <target>` | Locate a file, agent, or service |
| `koad map history` | Breadcrumb trail of recent locations |
| `koad map legend` | Symbol reference |

## Session Start Sequence

Run these three in order after `agent-boot`:

```bash
koad map look     # orient: where am I, what's here
koad map nearby   # surface: what tasks/configs are relevant
koad map exits    # scope: what paths exist
```

## When to Use Each

- **Unfamiliar directory** → `look` first
- **Looking for related files/tasks** → `nearby`
- **Switching projects** → `goto <alias>`
- **Can't find a file/agent** → `where <target>`

## Common Mistakes

| Mistake | Fix |
|---|---|
| Skipping map at session start | Run `look` + `nearby` before any work |
| Using `find` or `ls` for orientation | `koad map look` gives richer context |
| Hard-coding paths | Use `goto <alias>` for pinned locations |
