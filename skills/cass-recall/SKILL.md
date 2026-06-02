---
name: cass-recall
description: Use when starting a session with citadel-memory MCP available, re-orienting mid-session, or when lacking context about prior work — before rebuilding knowledge from scratch.
---

# CASS Recall

Orient to prior memory via CASS before rebuilding context from scratch.

## When to Use

- Session start when `citadel-memory` MCP is configured
- User asks about something you might already know from prior sessions
- You're about to summarize or reconstruct context you may have already stored

## Steps

**1. Verify CASS is online**

```
tool: status.citadel
params: {}
```

If offline: stop and report. Do not proceed as if memory is empty — it may just be unreachable.

**2. List known topics in your partition**

```
tool: memory.list_topics
params: {}
```

Scan the returned domains. This tells you what memory exists without fetching content.

**3. Pull recent memory cards**

```
tool: memory.recall
params: { "limit": 15 }
```

Read the cards. Surface anything relevant to the current session's likely scope.

## Report to User

- CASS status (ONLINE / OFFLINE)
- Topic domains found (list them)
- Any cards directly relevant to current work

## Common Mistakes

| Mistake | Fix |
|---|---|
| Skipping status check, assuming CASS is up | Always call `status.citadel` first |
| Treating empty recall as "no memory exists" | Empty partition ≠ no memory — CASS may be offline |
| Rebuilding context that's already in CASS | Run recall before summarizing anything |
