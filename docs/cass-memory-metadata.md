# CASS Token-Aware Memory Metadata

> Audience: KoadOS engineers and agent authors. This documents the optional metadata layer added to CASS memories so hydration packs better context per token. Canonical memory text is never changed by this feature — metadata is advisory.

## 1. Overview

CASS now distinguishes four roles for what it stores about a memory:

| Role | What it is | Purpose |
|------|-----------|---------|
| **Canonical text** | The fact/episode content | Source of truth — always plain, human-readable |
| **Embeddings / search data** | Vectors, content hashes | Retrieval signal (which memories are relevant) |
| **Token metadata** | Token estimates, hashes | Prompt-packing / cost signal (how expensive to inject) |
| **Budget hints** | Priority, injection mode, cache flags | Injection policy (whether and how to inject) |

CASS deliberately does **not** store:
- Raw token IDs as the canonical representation of a memory.
- Provider prompt-cache internals or handles.
- Opaque, model-specific blobs that make a memory unreadable.

The token metadata is model/tokenizer-scoped, recomputable, and optional. Old memories without metadata keep working unchanged.

## 2. The `MemoryMetadata` schema

Defined in `proto/cass.proto`, attached as an optional field on both `FactCard` (`metadata = 9`) and `EpisodicMemory` (`metadata = 7`). Persisted as JSON in the SQLite L2 tier and carried through the L1 Redis cache. (The metadata messages derive serde `Serialize`/`Deserialize`; timestamps are `int64` unix-epoch so the JSON stays portable.)

### `TokenEstimate`
| Field | Type | Notes |
|-------|------|-------|
| `tokenizer` | string | e.g. `cl100k_base`, `unknown_heuristic` |
| `model_hint` | string | optional model label |
| `tokens` | u32 | estimated token count |
| `method` | string | `library`, `heuristic`, `provider_usage` |
| `computed_at_unix` | i64 | epoch seconds; `0` = unset |

### `PromptBudgetHints`
| Field | Type | Notes |
|-------|------|-------|
| `max_prompt_tokens` | u32 | hard cap when injected |
| `preferred_prompt_tokens` | u32 | target budget |
| `injection_mode` | string | `verbatim`, `summary`, `title_only`, `never_auto` |
| `priority` | string | `critical`, `high`, `normal`, `low`, `archive` |
| `cache_stable` | bool | safe to place in the cache-friendly prompt prefix |
| `allow_compression` | bool | (proto3 default `false`; see Limitations) |

### `RetrievalMetadata`
| Field | Type | Notes |
|-------|------|-------|
| `salience` | f32 | durable usefulness, 0.0–1.0 |
| `recency_weight` | f32 | time-sensitive boost |
| `volatility` | string | `stable`, `mutable`, `ephemeral` |
| `audience` | string | `self`, `agent`, `team`, `global` |
| `scope` | string | project/user/platform scope |
| `expires_at_unix` | i64 | epoch seconds; `0` = never |

### `ProvenanceMetadata`
| Field | Type | Notes |
|-------|------|-------|
| `source` | string | `mcp`, `cli`, `backfill` |
| `source_ref` | string | message/session/file ref if safe |
| `author_agent` | string | committing agent |
| `content_hash` | string | sha256 of content |
| `schema_version` | string | metadata schema version |

### `PrivacyMetadata`
| Field | Type | Notes |
|-------|------|-------|
| `sensitivity` | string | `public`, `internal`, `private`, `secret-adjacent` |
| `contains_pii` | bool | |
| `contains_secret` | bool | |
| `redaction_tags` | repeated string | |

### `MemoryMetadata` (envelope)
Holds `token_estimates` (repeated), `prompt_budget`, `retrieval`, `provenance`, `privacy`, a short `summary` string (alternate injection text), and `metadata_json` — an escape hatch for future keys.

## 3. v1 Defaults (auto-population)

Every write funnels through the CASS gRPC service (`CommitFact`), which fills sensible defaults **once** before storage. When a card is committed without metadata, the service auto-populates:

- **Token estimate** — `tokenizer = cl100k_base`, `method = library` (real `tiktoken` count; falls back to a heuristic only if the tokenizer fails to load).
- **priority** — `normal`.
- **injection_mode** — `verbatim` if estimated tokens ≤ 80; else `summary` if a `summary` is present; else `verbatim`.
- **salience** — defaults to the card's `confidence`.
- **recency_weight** — `1.0`.
- **volatility** — `stable` if the domain topic is in `{identity, profile, project, convention, user}`, else `mutable`. (Topic = the segment after the last `:` in the domain.)
- **cache_stable** — `true` only when `volatility == stable`.
- **content_hash** — `sha256(content)`; **schema_version** — `"1"`; **author_agent** — the source agent.

Enrichment fills **only empty subfields** — any value you provide explicitly is preserved.

## 4. Overriding via `memory.commit`

The MCP `memory.commit` tool accepts optional metadata fields. `content` remains the **only** required input; anything you omit is auto-filled by the service.

Optional fields: `summary`, `priority`, `injection_mode`, `max_prompt_tokens`, `preferred_prompt_tokens`, `salience`, `volatility`, `sensitivity`.

```jsonc
// minimal — service backfills everything
{ "content": "Ian is Dood (admin/approver)." }

// enriched
{
  "content": "Skylinks pilot starts with Kimmie.",
  "topic": "rollout",
  "priority": "high",
  "injection_mode": "summary",
  "summary": "Pilot: Kimmie first.",
  "salience": 0.9,
  "volatility": "mutable",
  "sensitivity": "internal"
}
```

The card's dedup ID is `sha256(partition:content)` — metadata does not affect it, so re-committing the same content with different metadata is still idempotent on identity.

## 5. Backfilling legacy rows

Rows committed before this feature have `metadata_json IS NULL`. The `backfill_metadata` binary (in `koad-cass`) populates them using the **same** default derivation as the live service, so backfilled and freshly-written cards are consistent.

```bash
# dry-run (default): reports how many rows lack metadata + estimated token total, writes nothing
backfill_metadata --db <path-to-cass.db>

# scope to one partition (matches the domain prefix)
backfill_metadata --db <path> --partition hermes_jupiter_ideans

# actually write
backfill_metadata --db <path> --apply
```

It only ever writes `metadata_json`. It **never** changes `content`, `domain`, `tags`, or `confidence`. Running it twice is safe — already-populated rows are skipped.

## 6. Hydration behavior

During Temporal Context Hydration, the fact section is packed under the requested token budget:

1. **Ranking** — facts are ordered by priority tier (`critical` → `high` → `normal` → `low` → `archive`; missing = `normal`), then by **packing score**:

   ```
   score = confidence * salience * recency_weight / tokens
   ```

   Higher value-per-token is injected first.

2. **Per-fact budgeting** — each candidate line is checked against the running budget before inclusion, so a concise high-value fact wins over a long low-value one.

3. **Injection modes** — `never_auto` is skipped; `title_only` emits domain + confidence only; `summary` uses the `summary` text (falling back to content if empty); `verbatim` emits full content.

4. **Stable vs volatile split** — facts are emitted in two subsections:
   - **`## Ⅱ-A. Stable Fact Cards`** — `cache_stable == true`, in deterministic order (`domain`, priority, `id`), placed **first** so the prompt-cache prefix stays byte-stable across sessions.
   - **`## Ⅱ. Active Fact Cards`** — everything else, in ranked order.

   Legacy rows without metadata collapse to a single `## Ⅱ. Active Fact Cards` section — identical to pre-feature behavior.

## 7. Inspecting metadata

The MCP `memory.recall` and `memory.search_semantic` tools accept `include_metadata: true` (default `false`). When enabled, each card gets a compact debug line:

```
    └ meta: ~42tok prio=high mode=summary sal=0.75 vol=stable sens=private
```

Default output is unchanged — this is a debug aid, not normal prompt content. **Do not** inject raw metadata JSON into prompts.

## 8. v1 Limitations

- **L3 Qdrant does not round-trip metadata yet** (deferred to a future phase). The L2 SQLite tier is authoritative for metadata; a result that comes back via semantic search may carry empty metadata. Acceptable because metadata is advisory.
- **Token estimates are `cl100k_base` and advisory** — close for most models, not exact for every tokenizer. Exact per-provider tokenizers are a future phase.
- **proto3 zero/unset ambiguity** — a numeric field cannot distinguish "unset" from an explicit `0.0`. An explicit `salience` or `recency_weight` of `0.0` is therefore treated as unset and backfilled. `allow_compression` is left at its proto3 default (`false`) for the same reason and is not consumed by hydration in v1.
