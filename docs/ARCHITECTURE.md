# KoadOS Architecture Guide

KoadOS is a **programmatic-first agentic infrastructure** built for reliability and high-speed developer workflows.

## 1. The Core Components

### A. The CLI (`koad`)
A compiled Rust binary that provides the primary interface for users and agents. It handles:
- **Identity & Context**: Ingests project-specific memory during `boot`.
- **Knowledge Routing**: Interfaces with the local SQLite database.
- **Orchestration**: Dispatches long-running tasks to the Daemon.

### B. The Memory (`koad.db`)
A local **SQLite** database that stores:
- **Facts & Learnings**: Small, contextual knowledge snippets.
- **Project Index**: Maps local paths to roles and stacks.
- **Audit Logs**: Records all tool-use events for future harvesting.

### C. The Spine (`koad-daemon`)
A background service written in Rust that:
- **Watches Directories**: Uses the `notify` crate to track file changes in real-time.
- **Pre-computes Deltas**: Summarizes local activity so when an agent "wakes up", it already has an intuition of what changed.

## 2. The Skill Subsystem
Skills are external scripts (Python, JS, etc.) that the CLI dispatches to.
- They live in `~/.koad-os/skills/`.
- They are **stateless**. Any state should be written back to `koad.db` via the CLI.
- This allows anyone to extend Koad without recompiling the Rust core.

## 3. The Driver Subsystem
Drivers define how Koad communicates with specific LLM agents (e.g., Gemini, GPT-4).
- **`BOOT.md`**: A model-specific prompt that establishes the "Pillars" and "Directives".
- **Tool Mapping**: Drivers translate Koad's internal capabilities into agent-friendly tools.

---

**Philosophy:**
- **Simplicity Over Complexity**: If a feature can be a 10-line Python script, it should not be in the Rust core.
- **Plan Before Build**: Every major evolution starts with a `SPEC.md`.
- **Agnostic & Native**: KoadOS doesn't care which LLM you use, but it cares deeply about your local environment.
