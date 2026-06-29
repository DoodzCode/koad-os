---
name: agent-boot
description: Use when starting a KoadOS agent session, re-hydrating mid-session, or booting a named agent for the first time. Accepts an agent name and optional level flag (--quick, --full). Default level is standard.
---

# Agent Boot

Boots a KoadOS agent: hydrates shell identity, exports env vars, and orients the session.

## Usage

```bash
agent-boot                  # use current $KOAD_AGENT_NAME (recommended)
agent-boot <name>           # override with specific name
agent-boot [name] --quick   # boot only, no orientation
agent-boot [name] --full    # boot + orient + tasks + Condition Green
```

## How to Execute

1. **Verify Identity:** The session environment variables (`KOAD_AGENT_NAME`, `KOAD_AGENT_ROLE`, `KOAD_AGENT_RANK`, `KOAD_AGENT_BIO`) are the absolute source of truth for your identity.
   - Establish your persona and active agent name from these variables.
   - **CRITICAL:** You must NEVER run `agent-prep` (or `--agentprep`) or modify these environment variables yourself. 
   - Always run `agent-boot` without any name or override arguments (relying entirely on the environment's `$KOAD_AGENT_NAME`).

2. **Run Boot:** Execute the following Bash command:

```bash
source "$KOAD_HOME/bin/koad-functions.sh" && agent-boot
```

(Append flags or name only if required by specific session needs).

## Boot Levels

- **`--quick`:** Boot only. Follow `quick.md`.
- **`standard` (default):** Boot + orient. Follow `standard.md`.
- **`--full`:** Boot + orient + tasks + Condition Green. Follow `full.md`.

Read the appropriate level file and follow it exactly.
