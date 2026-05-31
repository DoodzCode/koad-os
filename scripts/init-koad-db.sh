#!/bin/bash
DB_PATH="$HOME/.koad-os/data/db/koad.db"

sqlite3 "$DB_PATH" <<SQL
CREATE TABLE IF NOT EXISTS identities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    bio TEXT,
    tier INTEGER DEFAULT 0
);

CREATE TABLE IF NOT EXISTS identity_roles (
    identity_id TEXT NOT NULL,
    role TEXT NOT NULL,
    PRIMARY KEY (identity_id, role),
    FOREIGN KEY (identity_id) REFERENCES identities(id)
);

CREATE TABLE IF NOT EXISTS knowledge (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    category TEXT NOT NULL,
    content TEXT NOT NULL,
    tags TEXT,
    timestamp TEXT NOT NULL,
    origin_agent TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS active_spec (
    title TEXT,
    description TEXT,
    status TEXT,
    priority TEXT
);

CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    path TEXT NOT NULL,
    branch TEXT,
    health TEXT
);

CREATE TABLE IF NOT EXISTS identity_snapshots (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    trigger TEXT NOT NULL,
    notes TEXT,
    created_at TEXT NOT NULL,
    origin_agent TEXT NOT NULL
);

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
SQL

echo "koad.db initialized at $DB_PATH"
