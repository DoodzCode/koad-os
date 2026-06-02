---
name: koad-fleet
description: Use when registering new projects, updating the GitHub Command Deck (Project Board), checking project status, publishing chronological code updates, or running Strategic Design Reviews (SDR).
---

# KoadOS Fleet & Project Management

Orchestration, task tracking, and coordination across KoadOS projects and the Command Deck.

## Core Workflows

### 1. Project Registration & Management (`koad project`)
Register and track project metadata to orient the agent and Admiral.
- **List registered projects:**
  ```bash
  koad project list
  ```
- **Register a new project root:**
  ```bash
  koad project register
  ```
- **Check project info/diagnostics:**
  ```bash
  koad project info
  ```
- **Sync project metadata & branches:**
  ```bash
  koad project sync
  ```

### 2. Command Deck & Board Coordination (`koad board`)
Sync tasks between GitHub and the Local Memory Bank.
- **Show current board items & status:**
  ```bash
  koad board status
  ```
- **Sync GitHub with Local Memory Bank (2-way):**
  ```bash
  koad board sync
  ```
- **Transition tasks:**
  ```bash
  koad board done    # Move to 'Done'
  koad board todo    # Move/reopen to 'Todo'
  ```
- **Strategic Design Review (SDR):**
  Execute a design review before beginning implementation of a project milestone:
  ```bash
  koad board sdr
  ```

### 3. Chronological Codebase Updates (`koad updates`)
Log and retrieve updates to maintain CASS context hydration.
- **Post a new update entry:**
  Run after completing a task or milestone:
  ```bash
  koad updates post
  ```
- **List recent updates:**
  ```bash
  koad updates list
  ```
- **Show details of a specific entry:**
  ```bash
  koad updates show <ID>
  ```
- **Hydrate memory/CASS via updates digest:**
  ```bash
  koad updates digest
  ```

## Fleet Coordination (`koad fleet`)
For fleet-level coordination of boards, projects, and issues:
```bash
koad fleet board    # High-level Command Deck
koad fleet project  # Workspace mapping
koad fleet issue    # Fleet atomic issue tracking
```

## Best Practices
- **Post Updates Chronologically:** Always run `koad updates post` after resolving a major task to keep the team and memory bank hydrated.
- **Sync Before Starting:** Run `koad board sync` at session start to ensure your local state is aligned with GitHub.
- **Run SDRs for Complex Tasks:** Before starting high-stakes milestones, execute `koad board sdr` to verify architectural design choices.
