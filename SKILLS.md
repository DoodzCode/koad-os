# KoadOS Skill System

Skills are specialized scripts that extend KoadOS functionality. Each coding agent can develop their own skills or share global skills.

## Directory Structure
- `~/.koad-os/skills/global/`: Shared skills available to all agents.
- `~/.koad-os/skills/<agent>/`: Agent-specific skills (e.g., `gemini/`, `claude/`).

## How to Build a Skill
1. **Create a Script**: Write a script in your preferred language (Python, Node.js, Rust, Bash).
2. **Make it Executable**: `chmod +x <script_path>`.
3. **Use the Koad CLI**: Skills can call the `koad` binary to access memory (`fact`), context (`boot`), or auth logic (`auth`).

### Example (Python)
```python
#!/usr/bin/env python3
import subprocess

# Add a fact via the CLI
subprocess.run(["koad", "remember", "fact", "New skill deployed."])
```

## Running Skills
Use the koad CLI as a dispatcher:
`koad skill run <category>/<skill_name> [args...]`

Example:
`koad skill run global/cleanup.sh --dry-run`

## Key Skills
- **gemini/remember.py**: Bi-directional memory bridge (Internal <-> SQLite).
- **gemini/harvest.py**: Context harvester and session logger.
- **gemini/search.py**: Surgical knowledge retrieval with token-limits.
- **gemini/detective.py**: High-speed project mapping and entry-point discovery.

## Guidelines
- **Simplicity**: Favor single-file scripts when possible.
- **Native Tech**: Use system-native interpreters (Python, Node, Bash).
- **Persistence**: Use `koad remember fact` to save results of skill execution to long-term memory.
