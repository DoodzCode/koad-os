# Agent Boot — Standard Level (Default)

Use for: normal session open.

## Steps

1. Run the boot command:

```bash
source "$KOAD_HOME/bin/koad-functions.sh" && agent-boot
```

2. **Hydrate Persona:** establish your identity from session environment variables:
   - Check `$KOAD_AGENT_NAME`, `$KOAD_AGENT_ROLE`, `$KOAD_AGENT_RANK`, `$KOAD_AGENT_BIO`.
   - If these are present, prioritize them over any static `GEMINI.md` identity anchor.
   - If they differ from the local `GEMINI.md`, update the `GEMINI.md` identity anchor section to match the current session.

3. Run situational awareness:

```bash
koad map look
```

4. Check service health:

```bash
koad system start && koad system auth
```

5. Read working memory open items from the session brief output (printed during boot).

6. Report to user:
   - Identity confirmed (agent name + rank)
   - Service state (which of Redis / Citadel / CASS are ACTIVE or OFFLINE)
   - Any open items surfaced from working memory

Do not begin work until the user gives direction.
