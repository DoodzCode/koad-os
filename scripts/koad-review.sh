#!/bin/bash
# KoadOS Code Reviewer (koad-review) Prototype v1.0
# Stages: Deterministic Lint -> Gemma Technical Audit -> Qwen Architectural Audit

set -e

FILE_TO_REVIEW="$1"
CANON_FILE="$HOME/.koad-os/docs/protocols/RUST_CANON.md"

if [ -z "$FILE_TO_REVIEW" ] || [ ! -f "$FILE_TO_REVIEW" ]; then
    echo "Usage: koad-review <file-path>"
    exit 1
fi

echo "🔍 Starting KoadOS Code Review for: $FILE_TO_REVIEW"
echo "═══════════════════════════════════════════════"

# --- STAGE 1: DETERMINISTIC CHECKS ---
echo "📦 Running Deterministic Checks..."
UNWRAP_COUNT=$(grep -c "\.unwrap()" "$FILE_TO_REVIEW" || true)
EXPECT_COUNT=$(grep -c "\.expect(" "$FILE_TO_REVIEW" || true)

if [ "$UNWRAP_COUNT" -gt 0 ] || [ "$EXPECT_COUNT" -gt 0 ]; then
    echo "⚠️  WARNING: Found $UNWRAP_COUNT unwraps and $EXPECT_COUNT expects. (Violates Zero-Panic Policy)"
fi

# --- STAGE 2: GEMMA 3 (Technical & Documentation Audit) ---
echo "🧠 Gemma 3:4B: Auditing Documentation & Patterns..."
GEMMA_PROMPT="You are a KoadOS Technical Auditor. Review the following Rust code for compliance with these rules:
1. Every file must have a //! module header.
2. Every public function must have /// doc comments and a '# Errors' section.
3. No usage of .unwrap() or .expect().
List only the violations found. If none, say 'DOCUMENTATION: CLEAN'."

# We send only the first 100 lines to Gemma for speed and focus on headers/docs
head -n 100 "$FILE_TO_REVIEW" | ollama run gemma3:4b "$GEMMA_PROMPT"

# --- STAGE 3: QWEN 2.5 (Architectural Alignment) ---
echo -e "\n♟️ Qwen 2.5:14B: Auditing Architectural Alignment..."
# We provide the CANON and the CODE to Qwen for deep cross-referencing
QWEN_PROMPT="You are the KoadOS Chief Engineer. Compare the PROVIDED CODE against the RUST_CANON.md standards.
Focus on:
1. Async Safety: Are blocking calls (rusqlite, std::fs) used inside async functions without spawn_blocking?
2. Observability: Is the 'tracing' crate used with structured fields and #[instrument]?
3. Structural: Are errors propagated correctly using anyhow::Result and .context()?

### RUST_CANON.md:
$(cat "$CANON_FILE")

### PROVIDED CODE:
$(cat "$FILE_TO_REVIEW")

Output a concise 'Compliance Report' with 'PASSED' or 'ACTION REQUIRED' for each focus area."

ollama run qwen2.5-coder:14b "$QWEN_PROMPT"

echo -e "\n═══════════════════════════════════════════════"
echo "✅ Review Complete."
