# Agent Boot — Standard Level (Default)

Use for: normal session open.

## Steps

1. Run the boot command:

```bash
source /home/ideans/.citadel-jupiter/bin/koad-functions.sh && agent-boot <name>
```

2. Run situational awareness:

```bash
koad map look
```

3. Check service health:

```bash
koad system status
```

4. Read working memory open items from the session brief output (printed during boot).

5. Report to user:
   - Identity confirmed (agent name + rank)
   - Service state (which of Redis / Citadel / CASS are ACTIVE or OFFLINE)
   - Any open items surfaced from working memory

Do not begin work until the user gives direction.
