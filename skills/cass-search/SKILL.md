---
name: cass-search
description: Use when about to reconstruct knowledge about a specific topic, answering "have we done this before", or looking up a known domain — before generating an answer from scratch.
---

# CASS Search

Retrieve targeted memory from CASS before reconstructing known context.

## When to Use

- "What do we know about X?"
- "Have we solved this before?"
- About to explain or summarize a topic that may already be in memory
- Need a specific domain's full card (architecture decision, pattern, identity)

## Decision

```
Known domain name?
  YES → intel.get (exact pull)
  NO  → memory.search_semantic (content search)
```

## Semantic Search (unknown domain)

```
tool: memory.search_semantic
params: {
  "query": "<natural language description of what you need>",
  "limit": 5
}
```

Use descriptive queries — the search matches against card content.

Good: `"gRPC Tonic endpoint builder pattern"`
Bad: `"grpc"`

## Domain Lookup (known domain)

```
tool: intel.get
params: { "domain": "<exact domain string>" }
```

Use the domain strings from `memory.list_topics` as input. Exact match — spelling matters.

## After Results

- Found relevant cards → use them, cite the domain
- No results → proceed with reconstruction, note that CASS had no prior record
- CASS offline → check status via `status.citadel`, report before proceeding

## Common Mistakes

| Mistake | Fix |
|---|---|
| Vague one-word queries to search_semantic | Use descriptive phrases matching card content |
| Guessing domain names for intel.get | Run `memory.list_topics` first to see exact strings |
| Ignoring empty results and rebuilding silently | Note explicitly that memory had no prior record |
