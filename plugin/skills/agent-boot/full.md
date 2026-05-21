# Agent Boot — Full Level

Use for: start of a new major session, post-incident recovery, inter-agent handoff.

## Steps

1. Run the boot command:

```bash
source "$KOAD_HOME/bin/koad-functions.sh" && agent-boot
```

2. Run situational awareness:

```bash
koad map look
```

3. Check service health:

```bash
koad system start && koad system auth
```

4. Read open tasks from the agent vault:

```bash
ls $KOAD_VAULT_PATH/tasks/
cat $KOAD_VAULT_PATH/tasks/*.md 2>/dev/null || echo "No open task files."
```

5. Assert Condition Green:
   - All required services (Redis, Citadel gRPC, CASS gRPC) must be ACTIVE
   - If any are OFFLINE, flag them explicitly before proceeding
   - Do not begin implementation work with degraded services unless Dood explicitly approves

6. Deliver full situational report to Dood:
   - Identity confirmed
   - Service state (GREEN / DEGRADED — list any OFFLINE services)
   - Open tasks (list titles)
   - Blockers (anything preventing Condition Green)
   - Ready for orders
