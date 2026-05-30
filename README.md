# KoadOS — The Frontier Agent Exoskeleton

KoadOS is a local-first capability layer designed to enhance frontier agent harnesses (Claude Code, Gemini CLI, Codex) with persistent memory, active workspace awareness, and compounding cognitive context.

**The Agents are the product.** KoadOS provides the infrastructure that allows frontier models to transcend the single-session limit, reducing token burn while increasing system-wide engineering accuracy.

## 🛡 Stable Release: v3.2.0 "Citadel Integrity"

The v3.2.0 release establishes the **Frontier Uplink Architecture**:
- **Capability Layer (Docker):** CASS (Citadel Agent Support System), Redis, and Qdrant provide the "Brain" enhancements.
- **Navigation Engine (Host):** `code-review-graph` provides real-time workspace topology to the agent.
- **Unified Harnessing:** Standardized environment and tool access across all supported agent CLIs.

---

## 🏗 Architecture: The Capability Stack

KoadOS works *with* your existing frontier agent tools:

1.  **Harness Layer:** You run `claude`, `gemini`, or `codex` as your primary interface.
2.  **KoadOS Layer (The Enhancer):**
    *   **Context:** `koad map` provides the agent with a high-fidelity "look" at the codebase via the knowledge graph.
    *   **Memory:** CASS records episodic and procedural facts, allowing the agent to "remember" previous sessions.
    *   **Governance:** The Citadel Kernel manages identities, permissions, and skill registration.

---

## 🚀 Getting Started

### Prerequisites:
*   **Rust** (Latest Stable) — [rustup.rs](https://rustup.rs)
*   **Docker** (Running) — Docker Desktop or Docker Engine + Compose plugin
*   **Python 3 & pipx** — `sudo apt install python3-pip pipx` on Ubuntu/WSL
*   **protoc** — `sudo apt install protobuf-compiler` on Ubuntu/WSL

### 1. Installation
```bash
# Clone the monorepo
git clone https://github.com/DoodzCode/koad-os.git ~/koados-citadel
cd ~/koados-citadel

# Build backend infrastructure and host binaries
# (also installs code-review-graph via pipx)
./install.sh
```

### 2. Initialization ("The Great Awakening")
```bash
# Set up your Citadel name, identity, and optional Claude Desktop agent
./koad-init.sh
```

### 3. Commanding the Crew
```bash
# Add KoadOS to your shell (~/.bashrc or ~/.zshrc)
export KOADOS_HOME="$HOME/.koad-os"
export PATH="$KOADOS_HOME/bin:$PATH"
source $KOADOS_HOME/bin/koad-functions.sh

# Reload shell
source ~/.bashrc

# Boot your Captain to begin operations
agent-boot captain
```

### 4. Claude Desktop Memory Agent (Optional)
If you opted in during `koad-init.sh`, start your memory agent:
```bash
cd ~/koados-citadel
./docker/rook/rook-up.sh
```
Then add the printed config snippet to your `claude_desktop_config.json`.

---

## 📜 The Canon
All development follows the **KoadOS Contributor Canon** and **RUST_CANON**. Methods are verified via KSRP (KoadOS Self-Review Protocol) and PSRP (Post-Session Reflection Protocol).

AIS documentation hub:
- `docs/ais/README.md` (AIS index and operating system docs)

AIS doctrine references:
- `docs/ais/protocols/TOKEN_EFFICIENCY.md`
- `docs/ais/protocols/SPEC_EVALUATION_DOCTRINE.md` (Officer+ spec-eval gate)

---
*Build small. Waste nothing. Know more. Trust the Canon.*
