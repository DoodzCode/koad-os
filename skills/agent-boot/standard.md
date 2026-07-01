# Agent Boot — Standard Level (Default)

Use for: normal session open.

## Steps

1. Run the boot command:

```bash
source "$KOAD_HOME/bin/koad-functions.sh" && agent-boot
```

2. **Hydrate Persona:** Establish your identity from the session environment variables:
   - Check `$KOAD_AGENT_NAME`, `$KOAD_AGENT_ROLE`, `$KOAD_AGENT_RANK`, `$KOAD_AGENT_BIO`.
   - These environment variables are the absolute source of truth. Prioritize them over any static local `GEMINI.md` identity file.
   - If they differ from the local `GEMINI.md`, update the `GEMINI.md` identity anchor section to match the current session environment.
   - Under no circumstances should the agent run `agent-prep` or modify environment variables to change its identity.

3. Run situational awareness:

```bash
koad map look
```

4. Check service health:

```bash
koad system status
```

- If all systems show **[PASS]**: proceed to step 5.
- If any show **[FAIL]** or **[WARN]**: run `koad doctor -f` to self-heal.
- If issues persist: run `koad system start` to attempt manual service recovery.

5. Read working memory open items from the session brief output (printed during boot).

6. Report to user:
   - Identity confirmed (agent name + rank)
   - Service state (which of Redis / Citadel / CASS are ACTIVE or OFFLINE)
   - Any open items surfaced from working memory

7. Doctrine check (Officer+ ranks):
   - For non-trivial tasks, run the Spec Evaluation Gate (SEG) before accepting execution.
   - Use doctrine: `docs/ais/protocols/SPEC_EVALUATION_DOCTRINE.md`.
   - Publish SEG output (clarity score, risks, ambiguities, acceptance contract, go/hold).

8. **Recall Before Rebuild:** before starting any task, check `koad updates list -n 5` or query intel for related prior work to ensure context continuity.

Do not begin work until the user gives direction.
