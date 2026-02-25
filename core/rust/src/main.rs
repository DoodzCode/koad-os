use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::env;
use std::process::Command;
use chrono::{Local, Duration};
use std::io::{BufRead, BufReader, Write};
use rusqlite::{params, Connection};

mod tui;
mod airtable;

use airtable::AirtableClient;

#[derive(Debug, Serialize, Deserialize)]
pub struct KoadConfig {
    pub version: String,
    pub identity: Identity,
    pub preferences: Preferences,
    pub drivers: HashMap<String, DriverConfig>,
    #[serde(default)]
    pub notion: NotionConfig,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct NotionConfig {
    #[serde(default)]
    pub mcp: bool,
    pub index: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Identity {
    pub name: String,
    pub role: String,
    pub bio: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Preferences {
    pub languages: Vec<String>,
    #[serde(default)]
    pub booster_enabled: bool,
    pub style: String,
    pub principles: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DriverConfig {
    pub bootstrap: String,
    #[serde(default)]
    pub mcp_enabled: bool,
    #[serde(default)]
    pub tools: Vec<String>,
}

#[derive(Parser)]
#[command(name = "koad")]
#[command(version = "2.4.1")]
#[command(about = "The KoadOS Control Plane")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    #[arg(short, long, global = true, default_value = "admin")]
    role: String,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize agent context for the current session.
    /// Loads identity, auth tokens, active project, and contextual memory.
    Boot {
        /// Select the agent driver to use (e.g., 'gemini').
        #[arg(short, long, default_value = "gemini")]
        agent: String,
        /// Ingest project-specific context (PROJECT_PROGRESS.md).
        #[arg(short, long)]
        project: bool,
        /// (Optional) Focus on a specific task ID.
        #[arg(short, long)]
        task: Option<String>,
        /// Output a compact pipe-delimited string for scripting.
        #[arg(short, long)]
        compact: bool,
    },
    /// Display active authentication tokens for GitHub and Google Drive based on current directory.
    Auth,
    /// Search the Koad Knowledge Base for a specific term or tag.
    Query { 
        /// Search term or tag.
        term: String,
        /// Maximum results to return (Token Efficiency).
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        /// (Optional) Filter by specific comma-separated tags.
        #[arg(short, long)]
        tags: Option<String>,
    },
    /// Commit a new fact or learning to the persistent SQLite database.
    Remember {
        #[command(subcommand)]
        category: MemoryCategory,
    },
    /// Record a personal reflection or interpretation (Persona Journaling).
    /// Used for agent self-alignment and long-term reasoning storage.
    Ponder {
        /// Reflection text.
        text: String,
        /// (Optional) Comma-separated tags.
        #[arg(short, long)]
        tags: Option<String>,
    },
    /// Manage and execute specialized KoadOS Skills (Python/JS/Rust).
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Scaffold new files or projects using standardized KoadOS templates.
    Template {
        #[command(subcommand)]
        action: TemplateAction,
    },
    /// Initialize a new KoadOS root configuration (~/.koad-os/koad.json).
    Init {
        /// Overwrite existing configuration.
        #[arg(short, long)]
        force: bool,
    },
    /// Harvest discoveries from external files or git history into contextual memory.
    Harvest {
        /// Path to the file to harvest discoveries from.
        #[arg(short, long)]
        path: Option<PathBuf>,
        /// Harvest learnings from recent git commits.
        #[arg(short, long)]
        git: bool,
    },
    /// Synchronize state between local SQLite and cloud sources (Airtable/Notion).
    Sync {
        #[command(subcommand)]
        source: SyncSource,
    },
    /// Manage Airtable bases and records directly.
    Airtable {
        #[command(subcommand)]
        action: AirtableAction,
    },
    /// Start the Koad background service (Cognitive Booster).
    Serve,
    /// Interact with the Koad Stream (Notion-backed communication channel).
    Stream {
        #[command(subcommand)]
        action: StreamAction,
    },
    /// Execute Google Cloud Platform operations (Deployments, Logs, Audits).
    Gcloud {
        #[command(subcommand)]
        action: GcloudAction,
    },
    /// Manage files and synchronization with Google Drive.
    Drive {
        #[command(subcommand)]
        action: DriveAction,
    },
    /// Mark a knowledge entry as inactive (soft delete).
    Retire {
        /// ID of the knowledge record.
        id: i64,
    },
    /// Archive the current session and capture learnings.
    /// Updates SESSION_LOG.md and harvests verified actions from history.
    Saveup {
        /// Short summary of the work completed.
        summary: String,
        /// Scope of work (e.g., project name).
        #[arg(short, long, default_value = "General")]
        scope: String,
        /// Comma-separated list of specific facts to remember.
        #[arg(short, long)]
        facts: Option<String>,
        /// Automatically harvest verified command executions from the last 4 hours.
        #[arg(short, long)]
        auto: bool,
    },
    /// Register the current directory as a project in the Koad ecosystem.
    /// Requires a .koad directory to be present.
    Scan {
        /// Path to scan (defaults to CWD).
        path: Option<PathBuf>,
    },
    /// Save a quick note or idea from Ian to the agent.
    Note {
        /// Note content.
        text: String,
    },
    /// Record a brainstorm or rant to the personal ledger.
    Brainstorm {
        /// Brainstorm or rant text.
        text: String,
        /// Mark as a 'rant' for categorical filtering.
        #[arg(short, long)]
        rant: bool,
    },
    /// List Ian's notes to the agent.
    Notes {
        /// Maximum notes to return.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// List recent brainstorms and rants.
    Brainstorms {
        /// Maximum brainstorms to return.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// Dispatch a command to the Koad daemon for background execution.
    Dispatch {
        /// The command to execute.
        command: String,
        /// (Optional) Arguments for the command.
        #[arg(short, long)]
        args: Option<String>,
    },
    /// Publish all local KoadOS changes to the remote origin (git push).
    Publish {
        /// Custom commit message.
        #[arg(short, long)]
        message: Option<String>,
    },
    /// Display information about the current KoadOS identity and environment.
    Whoami,
    /// Run a real-time TUI dashboard of the KoadOS state.
    Dash,
    /// Track or update the current active task specification.
    Spec {
        #[command(subcommand)]
        action: SpecAction,
    },
    /// Run a self-diagnostic check of the KoadOS environment.
    Diagnostic {
        /// Perform a full system check including skills and remote access.
        #[arg(short, long)]
        full: bool,
    },
    /// Display KoadOS developer and onboarding guides.
    Guide {
        /// Guide name (onboarding, development, architecture).
        topic: Option<String>,
    },
}

#[derive(Subcommand)]
enum SpecAction {
    /// Update the current task specification.
    Set {
        /// Task title.
        title: String,
        /// (Optional) Detailed description or plan.
        #[arg(short, long)]
        desc: Option<String>,
        /// Task status (Active, Paused, Complete).
        #[arg(short, long, default_value = "Active")]
        status: String,
    },
    /// Display the current task specification.
    Read,
    /// Clear the current task specification.
    Clear,
}

#[derive(Subcommand)]
enum DriveAction {
    /// List files in the configured Google Drive directory.
    List { 
        /// Include shared drive files.
        #[arg(short, long)] 
        shared: bool 
    },
    /// Download a file by its Drive ID.
    Download { 
        /// Google Drive file ID.
        id: String, 
        /// Destination path.
        #[arg(short, long)] 
        dest: Option<PathBuf> 
    },
    /// Sync local directories with Google Drive based on project config.
    Sync,
}

#[derive(Subcommand)]
enum GcloudAction {
    /// List GCP resources.
    List { 
        /// Resource type (functions, buckets, etc.).
        #[arg(short, long, default_value = "functions")] 
        resource: String 
    },
    /// Trigger a Cloud Build deployment for a named function.
    Deploy { 
        /// Function name.
        name: String 
    },
    /// Tail or fetch logs for a specific function.
    Logs { 
        /// Function name.
        name: String, 
        /// Max log entries to return.
        #[arg(short, long, default_value_t = 20)] 
        limit: u32 
    },
    /// Run a security and spend audit on the project.
    Audit { 
        /// Project alias (ops, prod, etc.).
        #[arg(short, long, default_value = "ops")] 
        project: String 
    },
}

#[derive(Subcommand)]
enum SyncSource {
    /// Sync with Airtable bases.
    Airtable { 
        /// Only sync table schemas, not record data.
        #[arg(short, long)] 
        schema_only: bool, 
        /// (Optional) Specific Base ID.
        #[arg(short, long)] 
        base_id: Option<String> 
    },
    /// Sync with Notion pages or databases.
    Notion { 
        /// Specific Notion Page ID.
        #[arg(short, long)] 
        page_id: Option<String>, 
        /// Specific Notion Database ID.
        #[arg(short, long)] 
        db_id: Option<String> 
    },
    /// Sync a resource by its shortcut name defined in koad.json.
    Named { 
        /// Shortcut name (e.g., 'koad', 'stream', 'memories').
        name: String 
    },
}

#[derive(Subcommand)]
enum AirtableAction {
    /// List records from an Airtable table.
    List {
        /// The Airtable Base ID.
        base_id: String,
        /// The Table Name.
        table_name: String,
        /// (Optional) Filter formula.
        #[arg(short, long)]
        filter: Option<String>,
        /// (Optional) Max records to return.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
    },
    /// Get a specific record by ID.
    Get {
        /// The Airtable Base ID.
        base_id: String,
        /// The Table Name.
        table_name: String,
        /// The Record ID.
        record_id: String,
    },
    /// Update a record.
    Update {
        /// The Airtable Base ID.
        base_id: String,
        /// The Table Name.
        table_name: String,
        /// The Record ID.
        record_id: String,
        /// JSON fields to update.
        fields: String,
    },
}

#[derive(Subcommand)]
enum StreamAction {
    /// Post a new message to the Koad Stream.
    Post {
        /// Message topic.
        topic: String,
        /// Message content.
        message: String,
        /// Message type (Log, Alert, Query, etc.).
        #[arg(short, long, default_value = "Log")]
        msg_type: String,
    },
    /// List recent messages from the Koad Stream.
    List {
        /// Number of messages to retrieve.
        #[arg(short, long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Subcommand)]
enum MemoryCategory {
    /// Record an objective fact.
    Fact { 
        /// Fact text.
        text: String, 
        /// Comma-separated tags.
        #[arg(short, long)] 
        tags: Option<String> 
    },
    /// Record a lesson learned or process improvement.
    Learning { 
        /// Learning text.
        text: String, 
        /// Comma-separated tags.
        #[arg(short, long)] 
        tags: Option<String> 
    },
}

#[derive(Subcommand)]
enum SkillAction {
    /// List all available KoadOS Skills.
    List,
    /// Execute a skill by name.
    Run { 
        /// Skill name (path relative to skills/).
        name: String, 
        /// Arguments passed to the skill.
        #[arg(last = true)] 
        args: Vec<String> 
    },
}

#[derive(Subcommand)]
enum TemplateAction {
    /// List all available templates.
    List,
    /// Apply a template to the current directory.
    Use { 
        /// Template name.
        name: String, 
        /// Destination path (defaults to name).
        #[arg(short, long)] 
        out: Option<PathBuf> 
    },
}

impl KoadConfig {
    pub fn get_home() -> Result<PathBuf> {
        env::var("KOAD_HOME")
            .map(PathBuf::from)
            .or_else(|_| dirs::home_dir().context("Home dir not found").map(|h| h.join(".koad-os")))
    }
    pub fn get_path() -> Result<PathBuf> { Ok(Self::get_home()?.join("koad.json")) }
    pub fn get_db_path() -> Result<PathBuf> { Ok(Self::get_home()?.join("koad.db")) }
    pub fn get_log_path() -> Result<PathBuf> { Ok(Self::get_home()?.join("SESSION_LOG.md")) }

    pub fn load() -> Result<Self> {
        let path = Self::get_path()?;
        if !path.exists() { return Ok(Self::default_initial()); }
        let content = std::fs::read_to_string(path)?;
        let mut cfg: Self = serde_json::from_str(&content).context("Failed to parse koad.json")?;
        if let Ok(val) = env::var("KOAD_NAME") { cfg.identity.name = val; }
        if let Ok(val) = env::var("KOAD_ROLE") { cfg.identity.role = val; }
        if let Ok(val) = env::var("KOAD_BIO") { cfg.identity.bio = val; }
        Ok(cfg)
    }

    pub fn save(&self) -> Result<()> {
        let path = Self::get_path()?;
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content).context("Failed to write koad.json")
    }

    pub fn default_initial() -> Self {
        Self {
            version: "2.4".to_string(),
            identity: Identity {
                name: env::var("KOAD_NAME").unwrap_or_else(|_| "Koad".into()),
                role: env::var("KOAD_ROLE").unwrap_or_else(|_| "Admin".into()),
                bio: env::var("KOAD_BIO").unwrap_or_else(|_| "Principal Systems & Operations Engineer; Agnostic AI framework.".into()),
            },
            preferences: Preferences {
                languages: vec!["Rust".into(), "Node.js".into(), "Python".into()],
                booster_enabled: false,
                style: "programmatic-first".to_string(),
                principles: vec![
                    "Simplicity first".into(), 
                    "Plan before build".into(),
                    "Sanctuary Rule".into()
                ],
            },
            drivers: HashMap::new(),
            notion: NotionConfig::default(),
        }
    }
}

pub struct KoadDB {
    pub conn: Connection,
}

impl KoadDB {
    pub fn init() -> Result<Self> {
        let path = KoadConfig::get_db_path()?;
        let conn = Connection::open(path)?;
        conn.execute("CREATE TABLE IF NOT EXISTS knowledge (id INTEGER PRIMARY KEY, category TEXT NOT NULL, content TEXT NOT NULL, tags TEXT, timestamp TEXT NOT NULL, active INTEGER DEFAULT 1)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS projects (id INTEGER PRIMARY KEY, name TEXT NOT NULL UNIQUE, path TEXT NOT NULL, role TEXT, stack TEXT, last_boot TEXT, active INTEGER DEFAULT 1)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS executions (id INTEGER PRIMARY KEY, command TEXT NOT NULL, args TEXT, timestamp TEXT NOT NULL, status TEXT)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS notion_index (id TEXT PRIMARY KEY, name TEXT, type TEXT, last_sync TEXT, cloud_edited TEXT, url TEXT)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS active_spec (id INTEGER PRIMARY KEY, title TEXT NOT NULL, description TEXT, status TEXT, last_update TEXT NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS ian_notes (id INTEGER PRIMARY KEY, content TEXT NOT NULL, timestamp TEXT NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS brainstorms (id INTEGER PRIMARY KEY, content TEXT NOT NULL, category TEXT NOT NULL, timestamp TEXT NOT NULL)", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS project_state (
            id INTEGER PRIMARY KEY,
            path TEXT NOT NULL,
            event_type TEXT NOT NULL,
            timestamp TEXT NOT NULL,
            summary TEXT
        )", [])?;
        conn.execute("CREATE TABLE IF NOT EXISTS command_queue (
            id INTEGER PRIMARY KEY,
            command TEXT NOT NULL,
            args TEXT,
            status TEXT NOT NULL DEFAULT 'pending',
            output TEXT,
            pid INTEGER,
            created_at TEXT NOT NULL,
            started_at TEXT,
            finished_at TEXT
        )", [])?;
        Ok(Self { conn })
    }

    pub fn set_spec(&self, title: &str, description: Option<String>, status: &str) -> Result<()> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute("INSERT INTO active_spec (id, title, description, status, last_update) VALUES (1, ?1, ?2, ?3, ?4) ON CONFLICT(id) DO UPDATE SET title=?1, description=?2, status=?3, last_update=?4", params![title, description, status, now])?;
        Ok(())
    }

    pub fn get_spec(&self) -> Result<Option<(String, Option<String>, String, String)>> {
        let mut stmt = self.conn.prepare("SELECT title, description, status, last_update FROM active_spec WHERE id = 1")?;
        let mut rows = stmt.query([])?;
        if let Some(row) = rows.next()? { Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))) } else { Ok(None) }
    }

    pub fn remember(&self, category: &str, content: &str, tags: Option<String>) -> Result<()> {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute("INSERT INTO knowledge (category, content, tags, timestamp) VALUES (?1, ?2, ?3, ?4)", params![category, content, tags, timestamp])?;
        Ok(())
    }

    pub fn save_note(&self, content: &str) -> Result<()> {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute("INSERT INTO ian_notes (content, timestamp) VALUES (?1, ?2)", params![content, timestamp])?;
        Ok(())
    }

    pub fn save_brainstorm(&self, content: &str, is_rant: bool) -> Result<()> {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let category = if is_rant { "rant" } else { "brainstorm" };
        self.conn.execute("INSERT INTO brainstorms (content, category, timestamp) VALUES (?1, ?2, ?3)", params![content, category, timestamp])?;
        Ok(())
    }

    pub fn dispatch(&self, command: &str, args: Option<String>) -> Result<()> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute("INSERT INTO command_queue (command, args, status, created_at) VALUES (?1, ?2, 'pending', ?3)", params![command, args, now])?;
        Ok(())
    }

    pub fn get_recent_brainstorms(&self, limit: usize) -> Result<Vec<(String, String, String)>> {
        let mut stmt = self.conn.prepare("SELECT content, category, timestamp FROM brainstorms ORDER BY timestamp DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        let mut results = Vec::new();
        for row in rows { results.push(row?); }
        Ok(results)
    }

    pub fn get_notes(&self, limit: usize) -> Result<Vec<(i64, String, String)>> {
        let mut stmt = self.conn.prepare("SELECT id, content, timestamp FROM ian_notes ORDER BY timestamp DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        let mut results = Vec::new();
        for row in rows { results.push(row?); }
        Ok(results)
    }

    pub fn get_ponderings(&self, limit: usize) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT content FROM knowledge WHERE category = 'pondering' AND active = 1 ORDER BY timestamp DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit], |row| Ok(row.get(0)?))?;
        let mut results = Vec::new();
        for row in rows { results.push(row?); }
        Ok(results)
    }

    pub fn log_execution(&self, command: &str, args: &str, status: &str) -> Result<()> {
        let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute("INSERT INTO executions (command, args, timestamp, status) VALUES (?1, ?2, ?3, ?4)", params![command, args, timestamp, status])?;
        Ok(())
    }

    pub fn get_recent_executions(&self, hours: i64) -> Result<Vec<(String, String, String)>> {
        let cutoff = (Local::now() - Duration::hours(hours)).format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = self.conn.prepare("SELECT command, args, status FROM executions WHERE timestamp > ?1 ORDER BY timestamp DESC LIMIT 15")?;
        let rows = stmt.query_map(params![cutoff], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        let mut results = Vec::new();
        for row in rows { results.push(row?); }
        Ok(results)
    }

    pub fn register_project(&self, name: &str, path: &str) -> Result<()> {
        let last_boot = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.conn.execute("INSERT INTO projects (name, path, last_boot) VALUES (?1, ?2, ?3) ON CONFLICT(name) DO UPDATE SET last_boot=?3, path=?2", params![name, path, last_boot])?;
        Ok(())
    }

    pub fn retire(&self, id: i64) -> Result<()> {
        self.conn.execute("UPDATE knowledge SET active = 0 WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn query(&self, term: &str, limit: usize, tags: Option<String>) -> Result<Vec<(i64, String, String, String)>> {
        let mut query_str = String::from("SELECT id, category, content, timestamp FROM knowledge WHERE active = 1");
        let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();
        
        if !term.is_empty() {
            query_str.push_str(" AND (content LIKE ?1 OR tags LIKE ?1)");
            params_vec.push(format!("%{}%", term).into());
        }

        if let Some(t) = tags {
            for tag in t.split(',') {
                query_str.push_str(" AND tags LIKE ?");
                params_vec.push(format!("%{}%", tag.trim()).into());
                query_str.push_str(&(params_vec.len()).to_string());
            }
        }

        query_str.push_str(" ORDER BY timestamp DESC LIMIT ?");
        params_vec.push((limit as i64).into());
        query_str.push_str(&(params_vec.len()).to_string());

        let mut stmt = self.conn.prepare(&query_str)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))?;
        
        let mut results = Vec::new();
        for row in rows { results.push(row?); }
        Ok(results)
    }

    pub fn get_contextual(&self, limit: usize, tags: Vec<String>) -> Result<Vec<(String, String)>> {
        let mut results = Vec::new();
        if !tags.is_empty() {
            let mut query = String::from("SELECT category, content FROM knowledge WHERE active = 1 AND category != 'pondering' AND (");
            for (i, _) in tags.iter().enumerate() {
                if i > 0 { query.push_str(" OR "); }
                query.push_str("tags LIKE ?"); query.push_str(&(i + 1).to_string());
                query.push_str(" OR content LIKE ?"); query.push_str(&(i + 1).to_string());
            }
            query.push_str(") ORDER BY timestamp DESC LIMIT ?");
            query.push_str(&(tags.len() + 1).to_string());
            let mut stmt = self.conn.prepare(&query)?;
            let mut params_vec: Vec<rusqlite::types::Value> = Vec::new();
            for t in &tags { params_vec.push(format!("%{}%", t).into()); }
            params_vec.push(((limit / 2) as i64).into());
            let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| Ok((row.get(0)?, row.get(1)?)))?;
            for row in rows { results.push(row?); }
        }
        let mut stmt = self.conn.prepare("SELECT category, content FROM knowledge WHERE active = 1 AND category != 'pondering' ORDER BY timestamp DESC LIMIT ?1")?;
        let rows = stmt.query_map(params![limit.saturating_sub(results.len())], |row| Ok((row.get(0)?, row.get(1)?)))?;
        for row in rows { 
            let item = row?;
            if !results.contains(&item) { results.push(item); }
        }
        Ok(results)
    }

    pub fn get_recent_deltas(&self, minutes: i64) -> Result<Vec<(String, String, String)>> {
        let cutoff = (Local::now() - Duration::minutes(minutes)).format("%Y-%m-%d %H:%M:%S").to_string();
        let mut stmt = self.conn.prepare("SELECT path, event_type, timestamp FROM project_state WHERE timestamp > ?1 ORDER BY timestamp DESC LIMIT 10")?;
        let rows = stmt.query_map(params![cutoff], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        let mut results = Vec::new();
        for row in rows { results.push(row?); }
        Ok(results)
    }

    pub fn get_active_project(&self, current_path: &str) -> Result<Option<(String, Option<String>, Option<String>)>> {
        let mut stmt = self.conn.prepare("SELECT name, role, stack FROM projects WHERE path = ?1 AND active = 1")?;
        let mut rows = stmt.query(params![current_path])?;
        if let Some(row) = rows.next()? { Ok(Some((row.get(0)?, row.get(1)?, row.get(2)?))) } else { Ok(None) }
    }

    pub fn get_notion_index(&self) -> Result<Vec<(String, String, String, String, String)>> {
        let mut stmt = self.conn.prepare("SELECT name, type, last_sync, cloud_edited, id FROM notion_index ORDER BY name ASC")?;
        let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?;
        let mut results = Vec::new();
        for row in rows { results.push(row?); }
        Ok(results)
    }
}

fn get_gh_pat_for_path(_path: &Path) -> (&'static str, &'static str) {
    // For OSS, we default to the personal PAT. 
    // Users can override this by setting KOAD_GH_PAT in their environment.
    ("GITHUB_PERSONAL_PAT", "Personal")
}

fn get_gdrive_token_for_path(_path: &Path) -> (&'static str, &'static str) {
    ("GDRIVE_PERSONAL_TOKEN", "Personal")
}

fn detect_context_tags(path: &Path) -> Vec<String> {
    let mut tags = Vec::new();
    let path_str = path.to_string_lossy().to_lowercase();
    if path_str.contains("rust") || path.join("Cargo.toml").exists() { tags.push("rust".into()); }
    if path_str.contains("node") || path.join("package.json").exists() { tags.push("node".into()); }
    tags
}

fn run_diagnostic(full: bool, config: &KoadConfig) -> Result<()> {
    let home = KoadConfig::get_home()?;
    println!("--- KoadOS Diagnostic ---\nKoad Home: {}", home.display());

    let mut files_missing = false;
    for file in ["koad.json", "koad.db", "SESSION_LOG.md"] {
        if home.join(file).exists() { println!("[PASS] {} exists", file); } 
        else { println!("[FAIL] {} is missing", file); files_missing = true; }
    }

    for var in ["KOAD_HOME", "KOAD_NAME", "KOAD_ROLE", "KOAD_BIO"] {
        match env::var(var) {
            Ok(val) => println!("[PASS] Env {} = {}", var, val),
            Err(_) => println!("[INFO] Env {} is not set (using defaults)", var),
        }
    }

    let db_status = match KoadDB::init() {
        Ok(db) => match db.conn.query_row("SELECT count(*) FROM knowledge", [], |r| Ok(r.get::<_, i64>(0)?)) {
            Ok(c) => format!("[PASS] Database connected (Knowledge entries: {})", c),
            Err(e) => format!("[FAIL] Database query failed: {}", e),
        },
        Err(e) => format!("[FAIL] Database connection failed: {}", e),
    };
    println!("{}", db_status);

    if full {
        println!("\n--- Full Integration Check ---");
        // Bridge Skills (Core if agent matches)
        for skill in ["gemini/remember.py", "gemini/harvest.py", "gemini/search.py"] {
            let status = if home.join("skills").join(skill).exists() { "[PASS]" } else { "[FAIL] (Core Skill)" };
            println!("{} Bridge skill {} found", status, skill);
        }

        // Notion (Optional)
        if config.notion.mcp {
            println!("[PASS] Notion Integration: Enabled (MCP)");
        } else {
            println!("[INFO] Notion Integration: Not Enabled (Optional)");
        }

        let current_dir = env::current_dir()?;
        let (pat_var, pat_label) = get_gh_pat_for_path(&current_dir);
        let auth_status = if env::var(pat_var).is_ok() { "[PASS]" } else { "[FAIL] (Core Auth)" };
        println!("{} Auth token {} ({}) is set", auth_status, pat_var, pat_label);

        println!("\n--- Tool Availability ---");
        // Core Tools
        for tool in ["git"] {
            let status = if Command::new(tool).arg("--version").output().is_ok() { "[PASS]" } else { "[FAIL] (CORE)" };
            println!("{} {} is available", status, tool);
        }

        // Stack Tools (Conditionally Core based on preferences)
        for lang in &config.preferences.languages {
            let tools = match lang.as_str() {
                "Python" => vec!["python3"],
                "Node.js" => vec!["node", "npm"],
                "Rust" => vec!["cargo"],
                _ => vec![],
            };
            for t in tools {
                let status = if Command::new(t).arg("--version").output().is_ok() { "[PASS]" } else { "[FAIL] (STACK)" };
                println!("{} {} is available", status, t);
            }
        }

        // Optional Tools
        for tool in ["gh"] {
            if Command::new(tool).arg("--version").output().is_ok() {
                println!("[PASS] {} is available (Optional)", tool);
            } else {
                println!("[INFO] {} is not installed (Optional)", tool);
            }
        }

        // Expansion: Cognitive Booster
        println!("\n--- Expansions ---");
        if config.preferences.booster_enabled {
            let daemon_path = home.join("bin/koad-daemon");
            if daemon_path.exists() {
                println!("[PASS] Cognitive Booster: Enabled & Installed");
                let is_running = Command::new("pgrep").arg("-f").arg("koad-daemon").output().map(|o| o.status.success()).unwrap_or(false);
                if is_running { println!("[PASS] koad-daemon is running"); }
                else { println!("[INFO] koad-daemon is NOT running (Use 'koad serve' to start)"); }
            } else {
                println!("[FAIL] Cognitive Booster: Enabled in config but binary missing at {}", daemon_path.display());
            }
        } else {
            println!("[INFO] Cognitive Booster: Disabled (Optional Expansion)");
        }
    }

    println!("\nStatus: {}", if files_missing { "DEGRADED" } else { "OPERATIONAL" });
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = KoadConfig::load()?;
    let db = KoadDB::init()?;
    let role = cli.role.to_lowercase();
    let is_admin = role == "admin";
    let is_pm = role == "pm";
    let has_privileged_access = is_admin || is_pm;

    match cli.command {
        Commands::Dash => tui::run_dash(&db)?,
        Commands::Spec { action } => {
            match action {
                SpecAction::Set { title, desc, status } => db.set_spec(&title, desc, &status)?,
                SpecAction::Read => {
                    if let Some((t, d, s, ts)) = db.get_spec()? {
                        println!("--- Active Spec [{}] ---", ts);
                        println!("Title:   {}", t);
                        println!("Status:  {}", s);
                        if let Some(desc) = d { println!("Detail:  {}", desc); }
                    } else { println!("No active task spec."); }
                },
                SpecAction::Clear => {
                    db.conn.execute("DELETE FROM active_spec WHERE id = 1", [])?;
                    println!("Spec cleared.");
                }
            }
        },
        Commands::Diagnostic { full } => run_diagnostic(full, &config)?,
        Commands::Guide { topic } => {
            let docs_dir = KoadConfig::get_home()?.join("docs");
            if let Some(t) = topic {
                // Try to find the file regardless of case
                let mut found = false;
                if docs_dir.exists() {
                    for entry in std::fs::read_dir(&docs_dir)? {
                        let entry = entry?;
                        let name = entry.path().file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
                        if name == t.to_lowercase() {
                            let content = std::fs::read_to_string(entry.path())?;
                            println!("{}", content);
                            found = true;
                            break;
                        }
                    }
                }
                if !found {
                    println!("Guide for '{}' not found in {}.", t, docs_dir.display());
                }
            } else {
                println!("--- KoadOS Developer & Onboarding Guides ---");
                if docs_dir.exists() {
                    for entry in std::fs::read_dir(docs_dir)? {
                        let entry = entry?;
                        if entry.path().extension().map_or(false, |e| e == "md") {
                            println!("- {}", entry.path().file_stem().unwrap().to_string_lossy().to_lowercase());
                        }
                    }
                } else {
                    println!("No guides found.");
                }
                println!("\nUsage: koad guide <topic>");
            }
        }
        Commands::Boot { agent, project, task: _, compact } => {
            let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let current_path_str = current_dir.to_string_lossy().to_string();
            let (pat_var, _) = get_gh_pat_for_path(&current_dir);
            let (drive_var, _) = get_gdrive_token_for_path(&current_dir);
            let tags = detect_context_tags(&current_dir);

            if compact {
                println!("I:{}|R:{}|G:{}|D:{}|T:{}", config.identity.name, config.identity.role, pat_var, drive_var, tags.join(","));
            } else {
                println!("<koad_boot>");
                println!("Identity: {} ({})", config.identity.name, config.identity.role);
                println!("Auth: GH={} | GD={}", pat_var, drive_var);
                
                if let Some(driver) = config.drivers.get(&agent) {
                    let b_path = driver.bootstrap.replace("~", &env::var("HOME").unwrap_or_default());
                    if let Ok(content) = std::fs::read_to_string(b_path) {
                        println!("\n[Bootstrap: {}]\n{}", agent, content);
                    }
                }

                if let Some((p_name, _, _)) = db.get_active_project(&current_path_str)? {
                    println!("\n[Active Project: {}]", p_name);
                }

                println!("\n[Persona Reflections]");
                let ponders = db.get_ponderings(3)?;
                if ponders.is_empty() { println!("- No active reflections."); }
                for p in ponders { println!("- {}", p); }

                println!("\n[Contextual Memory]");
                for (cat, content) in db.get_contextual(8, tags)? { println!("- [{}] {}", cat, content); }

                println!("\n[Ian's Notes]");
                let notes = db.get_notes(5)?;
                if notes.is_empty() { println!("- No new notes from Ian."); }
                for (_, content, ts) in notes { println!("- ({}) {}", ts, content); }

                if config.preferences.booster_enabled {
                    println!("\n[Spine Intelligence Deltas]");
                    // Query knowledge for delta tags since last boot (approx last 4 hours or since boot)
                    let deltas = db.query("", 5, Some("delta".to_string()))?;
                    if deltas.is_empty() {
                        println!("- Spine is quiet. No background events recorded.");
                    } else {
                        for (_, _, content, ts) in deltas {
                            println!("- ({}) {}", ts, content);
                        }
                    }

                    let file_deltas = db.get_recent_deltas(60)?;
                    if !file_deltas.is_empty() {
                        println!("\n[Booster File Deltas (Last 60m)]");
                        for (path, event, ts) in file_deltas {
                            let p = PathBuf::from(path);
                            println!("- {}: {} ({})", event, p.file_name().unwrap_or_default().to_string_lossy(), ts);
                        }
                    }
                }

                println!("\n[Notion Index Summary]");
                let index = db.get_notion_index()?;
                if index.is_empty() { println!("- No resources indexed."); }
                for (name, r_type, local, _, _) in index { println!("- {}: [{}] (Synced: {})", name, r_type, local); }

                if project {
                    let progress_path = current_dir.join("PROJECT_PROGRESS.md");
                    if progress_path.exists() {
                        let p = std::fs::read_to_string(progress_path)?;
                        if let Some(s) = p.find("## Snapshot") { println!("\n[Project Progress]\n{}", p[s..].trim()); }
                    }
                }
                println!("</koad_boot>");
            }
        }
        Commands::Auth => {
            let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let (p, _) = get_gh_pat_for_path(&current_dir);
            let (d, _) = get_gdrive_token_for_path(&current_dir);
            println!("GH:{} | GD:{}", p, d);
        }
        Commands::Query { term, limit, tags } => {
            for (id, cat, content, ts) in db.query(&term, limit, tags)? { println!("- ID:{} [{}] ({}) {}", id, cat, ts, content); }
        }
        Commands::Remember { category } => {
            if !has_privileged_access { anyhow::bail!("Access Denied."); }
            match category {
                MemoryCategory::Fact { text, tags } => db.remember("fact", &text, tags)?,
                MemoryCategory::Learning { text, tags } => db.remember("learning", &text, tags)?,
            }
            println!("Memory updated.");
        }
        Commands::Ponder { text, tags } => {
            let p_tags = format!("persona-journal,{}", tags.unwrap_or_default());
            db.remember("pondering", &text, Some(p_tags))?;
            println!("Reflection recorded in persona journal.");
        }
        Commands::Skill { action } => {
             let skills_dir = KoadConfig::get_home()?.join("skills");
             match action {
                 SkillAction::List => {
                     for entry in std::fs::read_dir(&skills_dir)? {
                         let entry = entry?;
                         if entry.path().is_dir() {
                             let cat = entry.file_name().to_string_lossy().to_string();
                             for s in std::fs::read_dir(entry.path())? { println!("- {}/{}", cat, s?.file_name().to_string_lossy()); }
                         }
                     }
                 },
                 SkillAction::Run { name, args } => {
                     let mut child = Command::new(skills_dir.join(&name)).args(&args).spawn()?;
                     let status = child.wait()?;
                     db.log_execution("skill run", &name, if status.success() { "success" } else { "failed" })?;
                 }
             }
        }
        Commands::Template { action } => {
            let t_dir = KoadConfig::get_home()?.join("templates/project_flow");
            match action {
                TemplateAction::List => { if t_dir.exists() { for e in std::fs::read_dir(t_dir)? { println!("- {}", e?.file_name().to_string_lossy()); } } }
                TemplateAction::Use { name, out } => {
                    let s = t_dir.join(&name);
                    let d = out.unwrap_or_else(|| PathBuf::from(&name));
                    std::fs::copy(s, d)?; println!("Template applied.");
                }
            }
        }
        Commands::Init { force } => {
            if !is_admin { anyhow::bail!("Admin only."); }
            let p = KoadConfig::get_path()?;
            if p.exists() && !force { anyhow::bail!("Exists."); }
            KoadConfig::default_initial().save()?; println!("Initialized.");
        }
        Commands::Harvest { path, git } => {
            if !has_privileged_access { anyhow::bail!("Access Denied."); }
            if git { println!("Scanning git history..."); }
            if let Some(p) = path {
                let r = BufReader::new(std::fs::File::open(&p)?);
                let mut d = false;
                for l in r.lines() {
                    let l = l?;
                    if l.starts_with("## Discoveries") { d = true; continue; }
                    if l.starts_with("## ") && d { break; }
                    if d && l.trim().starts_with("- ") { db.remember("learning", &l.trim()[2..], None)?; }
                }
            }
        }
        Commands::Airtable { action } => {
            if !has_privileged_access { anyhow::bail!("Access Denied."); }
            let pat = env::var("AIRTABLE_KOAD_PAT").context("AIRTABLE_KOAD_PAT not set")?;
            let client = AirtableClient::new(pat);
            
            match action {
                AirtableAction::List { base_id, table_name, filter, limit } => {
                    let records = client.list_records(&base_id, &table_name, filter, Some(limit))?;
                    println!("--- {} Records found ---", records.len());
                    for r in records {
                        println!("ID: {} | Fields: {}", r.id, r.fields);
                    }
                },
                AirtableAction::Get { base_id, table_name, record_id } => {
                    let record = client.get_record(&base_id, &table_name, &record_id)?;
                    println!("ID: {} | Fields: {}", record.id, record.fields);
                },
                AirtableAction::Update { base_id, table_name, record_id, fields } => {
                    let json_fields: Value = serde_json::from_str(&fields)?;
                    let record = client.update_record(&base_id, &table_name, &record_id, json_fields)?;
                    println!("Record {} updated successfully.", record.id);
                }
            }
        }
        Commands::Serve => {
            if !config.preferences.booster_enabled {
                anyhow::bail!("Cognitive Booster is disabled in koad.json. Enable it to use 'koad serve'.");
            }
            let daemon_path = KoadConfig::get_home()?.join("bin/koad-daemon");
            if !daemon_path.exists() { anyhow::bail!("koad-daemon binary not found."); }
            
            println!("Starting Koad Cognitive Booster in background...");
            Command::new(daemon_path)
                .spawn()
                .context("Failed to launch daemon")?;
            println!("[PASS] Daemon launched.");
        },
        Commands::Sync { source } => {
            if !has_privileged_access { anyhow::bail!("Access Denied."); }
            match source {
                SyncSource::Airtable { schema_only: _, base_id } => {
                    let pat = env::var("AIRTABLE_KOAD_PAT").context("AIRTABLE_KOAD_PAT not set")?;
                    let _client = AirtableClient::new(pat);
                    println!("Syncing Airtable (Native Rust Implementation)...");
                    if let Some(id) = base_id {
                        println!("Target Base: {}", id);
                        // Future: Ingest schema/records into koad.db
                    } else {
                        println!("No Base ID provided. Scanning configured projects for Airtable metadata...");
                    }
                }
                SyncSource::Notion { page_id, db_id } => {
                    if config.notion.mcp {
                        println!("Note: MCP is enabled. The sync skill will use MCP tools for the backend pass.");
                    }
                    let mut args = vec!["skill".to_string(), "run".to_string(), "global/notion_sync.py".to_string(), "--".to_string()];
                    if let Some(id) = page_id { args.push("--page-id".to_string()); args.push(id); }
                    if let Some(id) = db_id { args.push("--db-id".to_string()); args.push(id); }
                    Command::new(env::current_exe()?).args(args).spawn()?.wait()?;
                }
                SyncSource::Named { name } => {
                    let id = config.notion.index.get(&name).ok_or_else(|| anyhow::anyhow!("Not found"))?;
                    if config.notion.mcp && name == "koad" {
                         println!("DELEGATE: Use Notion MCP tool for named resource '{}' (ID: {}).", name, id);
                         return Ok(());
                    }
                    let mut args = vec!["skill".to_string(), "run".to_string(), "global/notion_sync.py".to_string(), "--".to_string()];
                    args.push("--page-id".to_string());
                    args.push(id.clone());
                    Command::new(env::current_exe()?).args(args).spawn()?.wait()?;
                }
            }
        }
        Commands::Stream { action } => {
            if config.notion.mcp {
                match action {
                    StreamAction::Post { topic, message, msg_type } => {
                        println!("DELEGATE: Use Notion MCP 'API-post-page' to post to Koad Stream (DB: {}). Topic: {}, Type: {}, Msg: {}", 
                                 config.notion.index.get("stream").unwrap_or(&"UNKNOWN".to_string()),
                                 topic, msg_type, message);
                    }
                    StreamAction::List { limit } => {
                        println!("DELEGATE: Use Notion MCP 'API-query-data-source' to list Koad Stream (DB: {}, Limit: {}).", 
                                 config.notion.index.get("stream").unwrap_or(&"UNKNOWN".to_string()),
                                 limit);
                    }
                }
                return Ok(());
            }
            let s_path = KoadConfig::get_home()?.join("skills/global/notion_stream.py");
            match action {
                StreamAction::Post { topic, message, msg_type } => {
                    Command::new(s_path).arg("post").arg(topic).arg(message).arg("--type").arg(msg_type).spawn()?.wait()?;
                }
                StreamAction::List { limit } => {
                    Command::new(s_path).arg("list").arg("--limit").arg(limit.to_string()).spawn()?.wait()?;
                }
            }
        }
        Commands::Gcloud { action } => {
            let mut args = vec!["skill".to_string(), "run".to_string(), "global/gcloud_ops.py".to_string(), "--".to_string()];
            match action {
                GcloudAction::List { ref resource } => { args.push("list".to_string()); args.push("--resource".to_string()); args.push(resource.clone()); }
                GcloudAction::Deploy { ref name } => { if !is_admin { anyhow::bail!("Restricted."); } args.push("deploy".to_string()); args.push("--name".to_string()); args.push(name.clone()); }
                GcloudAction::Logs { ref name, limit } => { args.push("logs".to_string()); args.push("--name".to_string()); args.push(name.clone()); args.push("--limit".to_string()); args.push(limit.to_string()); }
                GcloudAction::Audit { ref project } => { if !is_admin { anyhow::bail!("Restricted."); } args.push("audit".to_string()); args.push("--project".to_string()); args.push(project.clone()); }
            }
            Command::new(env::current_exe()?).args(args).spawn()?.wait()?;
        }
        Commands::Drive { action } => {
            let mut args = vec!["skill".to_string(), "run".to_string(), "global/gdrive_ops.py".to_string(), "--".to_string()];
            match action {
                DriveAction::List { shared } => { args.push("list".to_string()); if shared { args.push("--shared".to_string()); } }
                DriveAction::Download { id, dest } => { args.push("download".to_string()); args.push("--id".to_string()); args.push(id); if let Some(d) = dest { args.push("--dest".to_string()); args.push(d.to_string_lossy().to_string()); } }
                DriveAction::Sync => { if !has_privileged_access { anyhow::bail!("Restricted."); } args.push("sync".to_string()); }
            }
            Command::new(env::current_exe()?).args(args).spawn()?.wait()?;
        }
        Commands::Retire { id } => { if !is_admin { anyhow::bail!("Restricted."); } db.retire(id)?; }
        Commands::Saveup { summary, scope, facts, auto } => {
            if !has_privileged_access { anyhow::bail!("Access Denied."); }
            if let Some(f) = facts { for item in f.split(',') { db.remember("fact", item.trim(), Some(scope.clone()))?; } }
            if auto {
                let recent = db.get_recent_executions(4)?;
                for (cmd, args, _) in recent { db.remember("fact", &format!("Verified {} with: {}", cmd, args), Some(format!("auto,{}", scope)))?; }
            }
            let log_entry = format!("\n## {} - {}\n- Scope: {}\n- Project: {}\n", Local::now().format("%Y-%m-%d"), summary, scope, db.get_active_project(&env::current_dir().unwrap_or_default().to_string_lossy())?.map(|(n,_,_)| n).unwrap_or("General".into()));
            std::fs::OpenOptions::new().append(true).create(true).open(KoadConfig::get_log_path()?)?.write_all(log_entry.as_bytes())?;
            println!("Saveup complete.");
        }
        Commands::Scan { path } => {
            let t = path.unwrap_or_else(|| env::current_dir().unwrap_or(PathBuf::from(".")));
            println!("Scanning for Koad projects in {}...", t.display());
            
            // Use fd-find to locate all .koad directories quickly
            let output = Command::new("fdfind")
                .arg(".koad")
                .arg("--type").arg("d")
                .arg("--hidden")
                .arg("--absolute-path")
                .arg(&t)
                .output();

            match output {
                Ok(out) if out.status.success() => {
                    let paths = String::from_utf8_lossy(&out.stdout);
                    let mut count = 0;
                    for line in paths.lines() {
                        let koad_path = PathBuf::from(line);
                        let project_root = koad_path.parent().unwrap_or(&koad_path);
                        let name = project_root.file_name().unwrap_or_default().to_string_lossy();
                        
                        if db.register_project(&name, &project_root.to_string_lossy()).is_ok() {
                            println!("[PASS] Registered: {} ({})", name, project_root.display());
                            count += 1;
                        }
                    }
                    println!("Scan complete. {} project(s) registered.", count);
                },
                _ => {
                    // Fallback to single directory check if fd fails or is missing
                    if t.join(".koad").exists() {
                        db.register_project(&t.file_name().unwrap_or_default().to_string_lossy(), &t.to_string_lossy())?;
                        println!("Project registered.");
                    } else {
                        println!("[INFO] No .koad directories found.");
                    }
                }
            }
        }
        Commands::Note { text } => {
            db.save_note(&text)?;
            println!("Note saved to KoadDB.");
        }
        Commands::Brainstorm { text, rant } => {
            db.save_brainstorm(&text, rant)?;
            println!("{} saved to KoadDB.", if rant { "Rant" } else { "Brainstorm" });
        }
        Commands::Notes { limit } => {
            let notes = db.get_notes(limit)?;
            println!("--- Ian's Notes ---");
            if notes.is_empty() { println!("No notes found."); }
            for (id, content, ts) in notes { println!("[ID:{}] ({}) {}", id, ts, content); }
        }
        Commands::Brainstorms { limit } => {
            let items = db.get_recent_brainstorms(limit)?;
            println!("--- Ian's Brainstorms & Rants ---");
            if items.is_empty() { println!("No brainstorms found."); }
            for (content, cat, ts) in items { println!("[{}] ({}) {}", cat.to_uppercase(), ts, content); }
        }
        Commands::Dispatch { command, args } => {
            db.dispatch(&command, args)?;
            println!("Command dispatched to Koad Spine.");
        }
        Commands::Publish { message } => {
            if !is_admin { anyhow::bail!("Admin only."); }
            let h = KoadConfig::get_home()?;
            let m = message.unwrap_or_else(|| format!("KoadOS Sync - {}", Local::now().format("%Y-%m-%d %H:%M")));
            
            // Detect current branch
            let branch_output = Command::new("git").arg("-C").arg(&h).arg("rev-parse").arg("--abbrev-ref").arg("HEAD").output()?;
            let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

            Command::new("git").arg("-C").arg(&h).arg("add").arg(".").spawn()?.wait()?;
            Command::new("git").arg("-C").arg(&h).arg("commit").arg("-m").arg(&m).spawn()?.wait()?;
            Command::new("git").arg("-C").arg(&h).arg("push").arg("origin").arg(&branch).spawn()?.wait()?;
            println!("Published to branch: {}", branch);
        }
        Commands::Whoami => {
            let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let current_path_str = current_dir.to_string_lossy().to_string();
            let tags = detect_context_tags(&current_dir);
            let user = env::var("USER").unwrap_or_else(|_| "Partner".into());
            
            println!("--- KoadOS Partnership ---");
            println!("Partner:   {}", user);
            println!("Persona:   {} ({})", config.identity.name, config.identity.role);
            println!("Status:    Active / Booted");
            println!("Bio:       {}", config.identity.bio);
            
            if let Some((p_name, role, stack)) = db.get_active_project(&current_path_str)? {
                println!("\n[Current Project Context]");
                println!("Project:   {}", p_name);
                if let Some(r) = role { println!("Role:      {}", r); }
                if let Some(s) = stack { println!("Stack:     {}", s); }
            }
            
            println!("\n[Environment]");
            println!("Context:   {}", tags.join(", "));
        }
    }
    Ok(())
}
