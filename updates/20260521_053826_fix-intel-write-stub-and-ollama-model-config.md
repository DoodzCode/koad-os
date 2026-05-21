+++
id        = "upd_20260521_053826_fix-intel-write-stub-and-ollama-model-config"
timestamp = "2026-05-21T05:38:26.000000000+00:00"
author    = "clyde"
level     = "citadel"
category  = "fix"
summary   = "Fix koad intel read/write: implement commit_knowledge, add Ollama model config + fallback"
+++

Diagnosed and fixed two independent failures blocking `koad intel remember` and `koad intel query`.

## Root Causes

**1. `commit_knowledge` was a stub (admin.rs)**
`AdminService::commit_knowledge` returned `Ok(success: true)` unconditionally without writing anything. CLI printed "Memory updated via Citadel." on every call regardless. Facts were never persisted to `koad.db`.

**2. Ollama model `mistral` not installed**
`InferenceRouter::new_default()` hardcoded `OllamaClient::new(None, None)` → resolved to `"mistral"`. Mistral was not in the local Ollama registry. CASS's significance scoring and summarization failed with 404 on every call, causing writes to drop silently and queries to return empty results.

## Changes

### `crates/koad-citadel/src/services/admin.rs`
- Added `Arc<KoadDB>` field to `AdminService`; updated `new()` signature
- Implemented `commit_knowledge`: calls `koad_db.remember(category, content, tags, agent)` with agent name derived from session_id (`SID-{agent}-{uuid}`)
- Returns gRPC `Status::internal` on DB write failure instead of silent false success

### `crates/koad-citadel/src/kernel.rs`
- Added `KoadDB::new()` call opening `koad.db` (path from `config.storage.db_name`)
- Passes `Arc<KoadDB>` into `AdminService::new()`

### `crates/koad-intelligence/src/router.rs`
- `new_default()` reads `KOADOS_INTEL_MODEL` env var; falls back to `"mistral"` if unset
- `score()` catches Ollama errors, falls back to `1.0` (store everything when offline)
- `summarize()` catches Ollama errors, falls back to returning raw text unchanged

### `.env` (citadel instance)
- Added `KOADOS_INTEL_MODEL=mistral`

## Also Done
- Pulled `mistral` model into local Ollama registry (~4.1GB)
- Rebuilt and redeployed `koad-citadel` and `koad-cass` binaries

## Verified
- `koad intel remember fact "..."` writes to `koad.db.knowledge` (row count increments, content confirmed via sqlite3)
- `koad intel query "<term>"` returns matching facts
- Both koad-citadel (50051) and koad-cass (50052) running clean post-deploy
