# KoadOS Developer Guide

KoadOS is designed to be highly extensible. Its core is written in **Rust** for performance and safety, while its "Skills" are lightweight scripts in **Python** or **JavaScript**.

## 1. Creating a New Skill
Skills live in `~/.koad-os/skills/` and are organized by category (e.g., `global/`, `gemini/`).

### Writing a Python Skill
1. Create a file in `skills/global/my_skill.py`.
2. Ensure it's executable (`chmod +x`).

```python
#!/usr/bin/env python3
import sys
import os
from pathlib import Path

def main():
    # Koad scripts often call the koad CLI back for memory access
    koad_home = Path(os.getenv("KOAD_HOME", Path.home() / ".koad-os"))
    koad_bin = koad_home / "bin" / "koad"
    
    print("My Skill Executed!")
    print(f"Arguments: {sys.argv[1:]}")

if __name__ == "__main__":
    main()
```

3. Run it via the CLI:
```bash
koad skill run global/my_skill.py -- --arg1 val1
```

## 2. Contributing to the Rust Core
The Rust core handles:
- **CLI Parsing** (`clap`)
- **Memory Storage** (`SQLite`)
- **Daemon Orchestration** (`notify`, `tokio`)
- **TUI Dashboard** (`ratatui`)

### To build and test:
```bash
cd core/rust
cargo build --release
cargo test
```

## 3. Best Practices
- **Token Efficiency**: Koad is optimized for LLM context windows. Keep output concise.
- **Sanctuary Rule**: Developer agents should only modify files and documentation within the designated project context.
- **Explicit Planning**: Always draft a `SPEC.md` for major new features before implementing.

---

**Resources:**
- Check out `skills/global/hello_koad.py` for a template.
- Review `SPEC.md` for schema and protocol definitions.
