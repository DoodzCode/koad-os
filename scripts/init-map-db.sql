CREATE TABLE IF NOT EXISTS pins (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    alias TEXT NOT NULL UNIQUE,
    path TEXT NOT NULL,
    scope TEXT DEFAULT 'personal', -- shared | personal | station
    agent_id TEXT,                 -- NULL if shared
    created_at TEXT DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS navigation_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id TEXT NOT NULL,
    path TEXT NOT NULL,
    timestamp TEXT DEFAULT (datetime('now'))
);
