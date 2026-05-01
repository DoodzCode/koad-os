---
name: agent-boot
description: Use when starting a KoadOS agent session, re-hydrating mid-session, or booting a named agent for the first time. Accepts an agent name and optional level flag (--quick, --full). Default level is standard.
---

# Agent Boot

Boots a KoadOS agent: hydrates shell identity, exports env vars, and orients the session.

## Usage

```
agent-boot <name>           # standard (default)
agent-boot <name> --quick   # boot only, no orientation
agent-boot <name> --full    # boot + orient + tasks + Condition Green
```

## How to Execute

Run the following Bash command (must run in the terminal — not a subprocess):

```bash
source /home/ideans/.citadel-jupiter/bin/koad-functions.sh && agent-boot <name>
```

Replace `<name>` with the target agent (e.g., `clyde`, `tyr`).

## Boot Levels

- **`--quick`:** Boot only. Follow `quick.md`.
- **`standard` (default):** Boot + orient. Follow `standard.md`.
- **`--full`:** Boot + orient + tasks + Condition Green. Follow `full.md`.

Read the appropriate level file and follow it exactly.
