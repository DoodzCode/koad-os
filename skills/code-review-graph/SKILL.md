---
name: code-review-graph
description: Use when checking graph health after edits, analyzing change impact before a refactor, finding what calls a function, or understanding community structure in the codebase.
---

# code-review-graph

Persistent incremental knowledge graph of the codebase. Hook auto-updates the graph on every Edit/Write/Bash — agents interact for queries and health checks.

## Hook Behavior (Automatic)

The project hook runs silently after every tool use:
```
PostToolUse (Edit|Write|Bash) → code-review-graph update --skip-flows
SessionStart                  → code-review-graph status
```

Agents do not need to trigger updates manually.

## Commands Agents Use

### Check graph health
```bash
code-review-graph status
```
Shows: node/edge count, file count, languages, last updated commit.

### Analyze change impact
```bash
code-review-graph detect-changes
```
Run before large refactors to see what the graph thinks will be affected.

### Generate visualization
```bash
code-review-graph visualize   # produces graph.html
code-review-graph wiki        # produces markdown community docs
```

### Force full rebuild (after major restructure)
```bash
code-review-graph build
```

## MCP Tools (via `citadel-memory` or `code-review-graph serve`)

When configured as an MCP server, exposes graph queries as tools. Check available tools via `status.citadel` or MCP tool list at session start.

## Common Mistakes

| Mistake | Fix |
|---|---|
| Manually running `update` after edits | Hook handles it — don't double-trigger |
| Ignoring "graph is stale" warnings | Run `build` if stale warning persists after edits |
| Searching for callers with `grep` | Use `detect-changes` or graph MCP tools for impact analysis |
