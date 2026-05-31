-- Phase 1A: SQLite Hardening & L4 Procedural Memory
PRAGMA journal_mode=WAL;

-- L4 Procedural Memory (Patterns & Skills)
CREATE TABLE IF NOT EXISTS procedural_memory (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  agent_id TEXT NOT NULL,
  skill_name TEXT NOT NULL,
  pattern TEXT NOT NULL,
  success_count INTEGER DEFAULT 0,
  failure_count INTEGER DEFAULT 0,
  last_used DATETIME,
  created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- episodic_memories is created by koad-cass on first boot; pre-seed schema here for indexes
CREATE TABLE IF NOT EXISTS episodic_memories (
  session_id TEXT PRIMARY KEY,
  project_path TEXT NOT NULL,
  summary TEXT NOT NULL,
  turn_count INTEGER NOT NULL,
  timestamp TEXT NOT NULL,
  task_ids TEXT NOT NULL
);

-- Optimization Indexes
CREATE INDEX IF NOT EXISTS idx_episodic_time ON episodic_memories(session_id, timestamp);
CREATE INDEX IF NOT EXISTS idx_skill_pattern ON procedural_memory(skill_name, pattern);

-- Evolution: Importance Score for Phase 1C (Handles error if already exists)
-- Note: sqlite doesn't support 'IF NOT EXISTS' for columns directly in ALTER
-- This is handled by the application layer or a script check
