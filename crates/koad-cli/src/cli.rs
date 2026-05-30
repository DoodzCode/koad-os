use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "koad")]
#[command(
    about = "The KoadOS Control Plane: Orchestrating the Citadel Grid",
    long_about = "The primary interface for KoadOS agents and the Admiral. Manages session lifecycle, intellectual memory, and system-wide orchestration within the Citadel environment."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    /// Set the session authorization tier (admiral, captain, officer, crew).
    /// If omitted, KoadOS will attempt to auto-resolve based on the agent name.
    #[arg(short, long, default_value = "admin")]
    pub role: String,

    /// Bypass pre-flight system integrity checks. Use only during recovery.
    #[arg(long)]
    pub no_check: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Wake a KAI and initialize a neural link session.
    Boot {
        /// The name of the agent to wake (e.g., Sky, Pippin).
        #[arg(short, long)]
        agent: String,

        /// Enable path-aware project context detection.
        #[arg(short, long)]
        project: bool,

        /// Target a specific task ID for the session.
        #[arg(short, long)]
        task: Option<String>,

        /// Output session metadata in a compact, pipe-friendly format.
        #[arg(short, long)]
        compact: bool,

        /// Token budget for initial context hydration. [default: 4000]
        #[arg(short, long, default_value_t = 4000)]
        budget: u32,

        /// Force boot: take over an existing session for the same agent (Sovereign agents only).
        /// Use when a prior session is orphaned and cannot be cleanly logged out.
        #[arg(long)]
        force: bool,

        /// Output only bare `export KEY=VAL` lines suitable for `eval`. Skips MOTD.
        #[arg(long)]
        export_env: bool,
    },

    /// Core system management, orchestration, and recovery.
    System {
        #[command(subcommand)]
        action: SystemAction,
    },

    /// Intellectual memory operations and knowledge retrieval.
    Intel {
        #[command(subcommand)]
        action: IntelAction,
    },

    /// Fleet-wide project coordination and board synchronization.
    Fleet {
        #[command(subcommand)]
        action: FleetAction,
    },

    /// Integration bridges for cloud ecosystems (GCP, Airtable, Notion).
    Bridge {
        #[command(subcommand)]
        action: BridgeAction,
    },

    /// Automated code review and Canon compliance audit.
    Review {
        /// The file path to review.
        file: PathBuf,
    },

    /// Asynchronous agent-to-agent messaging (A2A-S).
    Signal {
        #[command(subcommand)]
        action: SignalAction,
    },

    /// KoadOS Field Guide: Protocols, Prime Directives, and Quick Start.
    Guide {
        /// Topic: quick, canon, workflow, ais.
        topic: Option<String>,
    },

    /// Manage agent Experience Points (XP) and Skills.
    Xp {
        #[command(subcommand)]
        action: XpCommands,
    },

    /// Display the version of the KoadOS CLI and Citadel kernel.
    Version,

    /// Display real-time system telemetry and Citadel integrity.
    Status {
        /// Output telemetry data as JSON.
        #[arg(short, long)]
        json: bool,

        /// Perform an exhaustive diagnostic sweep (Ghost detection, Resource allocation).
        #[arg(short, long)]
        full: bool,
    },

    /// Perform a comprehensive system health check and self-healing sweep.
    Doctor {
        /// Attempt to fix any identified minor issues.
        #[arg(short, long)]
        fix: bool,

        /// Perform a deep GPU and Ollama offloading verification.
        #[arg(short, long)]
        gpu: bool,
    },

    /// Manage and sync the GitHub Command Deck (Project Board).
    Board {
        #[command(subcommand)]
        action: BoardAction,
    },

    /// High-level project mapping and registration.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },

    /// Reveal active agent persona, bio, and authorization rank.
    Whoami,

    /// Perform a deep audit of the agent's internal cognitive layers.
    Cognitive,

    /// Navigation Map: Contextual overview and fast-travel bookmarks.
    Map {
        #[command(subcommand)]
        action: Option<MapAction>,
        /// Enable verbose output for human captains.
        #[arg(short, long)]
        verbose: bool,
    },

    /// Gracefully logout and untether the current session.
    Logout {
        /// Explicit session ID to terminate.
        #[arg(short, long)]
        session: Option<String>,
    },

    /// Manage KoadOS Agent Identities (KAI).
    Agent {
        #[command(subcommand)]
        action: AgentAction,
    },

    /// Secure, command-based access layer for agent identity and credentials.
    Vault {
        #[command(subcommand)]
        action: VaultAction,
    },

    /// Chronological codebase updates board — post, list, and hydrate changes.
    Updates {
        #[command(subcommand)]
        action: UpdatesAction,
    },

    /// Broadcast a short-lived signal to all agents at session boot.
    Pulse {
        /// The pulse message to broadcast.
        message: Option<String>,
        /// Filter pulses by agent role (default: global).
        #[arg(long)]
        role: Option<String>,
        /// List active pulses instead of broadcasting.
        #[arg(long)]
        list: bool,
    },

    /// Run a command in an isolated Docker/Podman sandbox container.
    Sandbox {
        /// The shell command to execute inside the container.
        #[arg(trailing_var_arg = true)]
        command: Vec<String>,
        /// Container image to use (default: alpine:3.19).
        #[arg(long, default_value = "alpine:3.19")]
        image: String,
        /// Allow network access (default: off).
        #[arg(long)]
        network: bool,
        /// Memory limit (e.g. 64m).
        #[arg(long, default_value = "64m")]
        memory: String,
        /// Use podman instead of docker.
        #[arg(long)]
        podman: bool,
    },

    /// Deploy and scaffold KoadOS workspace components (Station, Outpost).
    Deploy {
        #[command(subcommand)]
        action: DeployAction,
    },

    /// Alias for `koad system save` — Sovereign Save Protocol (Total State Checkpoint).
    Saveup {
        /// Create a full durable backup (Database + Git commit).
        #[arg(short, long)]
        full: bool,
    },
}

#[derive(Subcommand)]
pub enum DeployAction {
    /// Scaffold a new KoadOS Station in the current directory.
    Station {
        /// Station name.
        name: String,
    },
    /// Scaffold a new KoadOS Outpost in the current directory.
    Outpost {
        /// Outpost name.
        name: String,
    },
}

#[derive(Subcommand)]
pub enum VaultAction {
    /// Return current agent identity and vault root.
    Whoami,
    /// Export vault-derived environment variables.
    Env,
    /// Read a file/value from the agent's vault sanctuary.
    Get {
        /// Relative path inside the vault.
        path: String,
    },
    /// List vault contents (skills, config, secrets).
    Ls {
        /// Optional relative path to list.
        path: Option<String>,
    },
    /// Manage scoped agent credentials.
    Secret {
        #[command(subcommand)]
        action: VaultSecretAction,
    },
    /// Discover, inspect, and manage skills for the active agent's vault.
    Skill {
        #[command(subcommand)]
        action: VaultSkillAction,
    },
    /// Synchronize vault state from Citadel. [STUB]
    Sync,
    /// Create a ghost config bundle for offline use. [STUB]
    Pack,
    /// Show vault connection status and cache freshness. [STUB]
    Status,
}

#[derive(Subcommand)]
pub enum VaultSecretAction {
    /// Read a scoped secret (API key, token).
    Get { key: String },
    /// Write a secret (Operator only).
    Set { key: String, value: String },
    /// List available secret keys (not values).
    Ls,
}

#[derive(Subcommand)]
pub enum VaultSkillAction {
    /// List all skills currently equipped by the active agent.
    #[command(alias = "ls")]
    List,
    /// Show detailed info about a specific equipped skill.
    Info {
        /// Blueprint ID of the skill to inspect.
        id: String,
    },
    /// Search for available skill blueprints not yet equipped.
    Search,
    /// Equip a skill blueprint to the active agent's identity.
    Equip {
        /// Blueprint ID to equip (from `skill search`).
        id: String,
    },
    /// Persist current skill XP state back to the identity TOML.
    Sync,
}

#[derive(Subcommand)]
pub enum AgentAction {
    /// Register a new KoadOS Agent Identity (KAI) and scaffold their KAPV vault.
    New {
        /// Agent name (e.g. "Clyde"). Will be lowercased as the identity key.
        name: String,

        /// Agent rank (Officer, Engineer, Crew, Specialist, Captain).
        #[arg(short = 'k', long, default_value = "Officer")]
        rank: String,

        /// Agent role description. Optional if config/identities/<key>.toml already exists.
        #[arg(short = 'r', long)]
        role: Option<String>,

        /// Agent bio (short narrative description). Optional if config/identities/<key>.toml already exists.
        #[arg(short = 'b', long)]
        bio: Option<String>,

        /// Required runtime body (claude, gemini, codex).
        #[arg(long, default_value = "claude")]
        runtime: String,

        /// Vault path override. Defaults to agents/<key> within the KoadOS home directory.
        #[arg(long)]
        vault: Option<String>,

        /// Comma-separated list of access key names (e.g. GITHUB_PAT).
        #[arg(long, default_value = "")]
        access_keys: String,

        /// Agent tier level (1=Initiate, 2=Crew, 3=Officer, 4=Captain).
        #[arg(long, default_value_t = 3)]
        tier: u32,

        /// Preview all changes without writing any files.
        #[arg(long)]
        dry_run: bool,
    },

    /// List all registered agent identities.
    List,

    /// Show identity details for a specific agent.
    Info {
        /// Agent name to inspect.
        agent: String,
    },

    /// Verify an agent's KAPV vault structure and auto-heal missing directories.
    Verify {
        /// Agent name to verify.
        agent: String,
    },
}

#[derive(Subcommand)]
pub enum MapAction {
    /// Describe the current directory and nearby points of interest.
    Look,
    /// Show available paths, siblings, and pinned connections.
    Exits,
    /// Navigate to a pinned location or alias.
    Goto {
        /// Alias or path to navigate to.
        target: String,
    },
    /// Bookmark a location as a favorite (pin).
    Pin {
        /// Alias for the bookmark.
        alias: String,
        /// Path to bookmark (defaults to current dir).
        path: Option<String>,
        /// Scope: 'personal' (default) or 'shared'.
        #[arg(short, long, default_value = "personal")]
        scope: String,
    },
    /// List all bookmarked locations.
    Pins,
    /// Show contextually relevant items (tasks, KAPVs, configs) based on current location.
    Nearby,
    /// Show the breadcrumb trail of recently accessed locations.
    History,
    /// Display a legend for all map symbols and markers.
    Legend,
    /// Show a region-specific health and status overlay.
    MapStatus,
    /// Locate a specific entity (file, page, agent, or service).
    Where {
        /// Search query.
        entity: String,
    },
}

#[derive(Subcommand)]
pub enum XpCommands {
    /// View current XP, level, and trust tier.
    Status {
        /// Optional: Name of the agent to query (defaults to self).
        agent: Option<String>,
    },
    /// Programmatically award XP to an agent (Admin/Captain only).
    Award {
        /// Target agent name.
        agent: String,
        /// Amount of XP to award.
        amount: i32,
        /// Reason for the award.
        reason: String,
        /// Source type: task | skill | system.
        #[arg(short, long, default_value = "system")]
        source: String,
    },
}

#[derive(Subcommand)]
pub enum SystemAction {
    /// Initialize a new KoadOS environment in the current path.
    Init {
        /// Force initialization, overwriting existing configs.
        #[arg(short, long)]
        force: bool,
    },

    /// Display active credentials and path-aware PAT mapping.
    Auth,

    /// Inspect or modify the global system configuration.
    Config {
        #[command(subcommand)]
        action: Option<ConfigAction>,

        /// Output configuration as JSON.
        #[arg(short, long)]
        json: bool,
    },

    /// Rebuild and redeploy the KoadOS core from source.
    Refresh {
        /// Restart Citadel services after successful build.
        #[arg(short, long)]
        restart: bool,
        /// Explicit confirmation to bypass the safety gate.
        #[arg(long)]
        confirm: bool,
    },

    /// Execute the Sovereign Save Protocol (Total State Checkpoint).
    Save {
        /// Create a full durable backup (Database + Git commit).
        #[arg(short, long)]
        full: bool,
    },

    /// Apply an atomic, surgical patch to a file.
    Patch {
        /// Target file path.
        path: Option<PathBuf>,
        /// Regex search pattern.
        #[arg(short, long)]
        search: Option<String>,
        /// Replacement string.
        #[arg(short, long)]
        replace: Option<String>,
        /// JSON payload for bulk patching.
        #[arg(long)]
        payload: Option<String>,
        /// Enable fuzzy matching for complex diffs.
        #[arg(short, long)]
        fuzzy: bool,
        /// Show changes without modifying the file system.
        #[arg(long)]
        dry_run: bool,
    },

    /// Perform a 5-pass cognitive efficiency audit.
    Tokenaudit {
        /// Remove audit artifacts after completion.
        #[arg(short, long)]
        cleanup: bool,
    },

    /// Spawn a new GitHub issue using a system template.
    Spawn {
        /// Template name (e.g., bug, feature, research).
        #[arg(short, long, default_value = "feature")]
        template: String,
        /// Issue title.
        #[arg(short, long)]
        title: String,
        /// Complexity weight (trivial, standard, complex).
        #[arg(short, long, default_value = "standard")]
        weight: String,
        /// Describe the high-level goal or problem.
        #[arg(short, long)]
        objective: Option<String>,
        /// Define the specific architectural or functional scope.
        #[arg(short, long)]
        scope: Option<String>,
        /// Specific labels to apply.
        #[arg(short, long)]
        labels: Vec<String>,
    },

    /// Bulk import data (Markdown/CSV) into KoadOS subsystems.
    Import {
        /// Source file path.
        source: PathBuf,
        /// Data format (md, csv). [default: md]
        #[arg(short, long, default_value = "md")]
        format: String,
        /// Custom regex delimiter for chunking.
        #[arg(short, long)]
        delimiter: Option<String>,
        /// Destination route (github-issues, hydration).
        #[arg(short, long, default_value = "github-issues")]
        route: ImportRoute,
        /// Issue template to use (for github-issues route).
        #[arg(short, long)]
        template: Option<String>,
        /// Labels to apply to imported items.
        #[arg(short, long)]
        labels: Vec<String>,
        /// Preview changes without persisting.
        #[arg(long)]
        dry_run: bool,
    },

    /// Acquire a distributed lock on a specific system sector.
    Lock {
        /// The sector or resource name to lock.
        sector: String,
        /// TTL in seconds for the lock. [default: 300]
        #[arg(short, long, default_value_t = 300)]
        ttl: u64,
    },

    /// Release a distributed lock on a specific system sector.
    Unlock {
        /// The sector or resource name to unlock.
        sector: String,
    },

    /// List all active distributed locks.
    Locks,

    /// Re-establish a neural link session after a disconnection or reboot.
    Reconnect {
        /// Explicit agent name to recover.
        #[arg(short, long)]
        agent: Option<String>,

        /// Perform a live re-sync using the active environment session ID.
        #[arg(short, long)]
        live: bool,
    },

    /// Trigger a manual backup of the system memory sectors.
    Backup {
        /// Sector to backup (all, sqlite, redis). [default: all]
        #[arg(short, long, default_value = "all")]
        source: String,
    },

    /// Tail or filter KoadOS log files.
    Logs {
        /// The service or component to filter by (e.g., citadel, cass, gateway).
        #[arg(short, long)]
        service: Option<String>,
        /// Number of lines to show from the end. [default: 50]
        #[arg(short, long, default_value_t = 50)]
        tail: usize,
        /// Follow log output in real-time.
        #[arg(short, long)]
        follow: bool,
    },

    /// Start the Citadel kernel and all dependent services (CASS).
    Start,

    /// Display real-time system telemetry and Citadel integrity.
    Status {
        /// Output telemetry data as JSON.
        #[arg(short, long)]
        json: bool,

        /// Perform an exhaustive diagnostic sweep (Ghost detection, Resource allocation).
        #[arg(short, long)]
        full: bool,
    },

    /// Restart the Citadel kernel and all dependent services.
    Restart,

    /// Gracefully stop the Citadel kernel and all background services.
    Stop {
        /// Trigger a full state drain before stopping.
        #[arg(short, long)]
        drain: bool,
        /// Explicit confirmation to bypass the safety gate.
        #[arg(long)]
        confirm: bool,
    },

    /// Maintain an active neural link session via periodic heartbeats.
    Heartbeat {
        /// Run as a background daemon process.
        #[arg(short, long)]
        daemon: bool,
        /// Target session ID (omit to use environment).
        #[arg(short, long)]
        session: Option<String>,
    },

    /// Prepare the Citadel for clean distribution — removes local state, logs, and databases.
    Scrub {
        /// Preview all targets without deleting anything.
        #[arg(long)]
        dry_run: bool,
        /// Skip the confirmation prompt (DANGER: immediately deletes targets).
        #[arg(long)]
        force: bool,
    },

    /// Manage and hydrate an agent's transient context.
    Context {
        #[command(subcommand)]
        action: ContextAction,
    },

    /// Force a 2-way synchronization between local reality (updates/log) and GitHub Project #6.
    BoardSync {
        /// Preview changes without modifying GitHub or CASS.
        #[arg(long)]
        dry_run: bool,
        /// Automatically create missing issues for local updates.
        #[arg(long)]
        auto_spawn: bool,
    },
}

#[derive(Subcommand)]
pub enum ContextAction {
    /// Inject a file or raw text into an agent's hot context.
    Hydrate {
        /// Target session ID (omit for current).
        #[arg(short, long)]
        session: Option<String>,
        /// Path to a file to hydrate.
        #[arg(short, long)]
        path: Option<PathBuf>,
        /// Raw text to hydrate (if path is omitted).
        #[arg(short, long)]
        text: Option<String>,
        /// TTL in seconds (0 = session-persistent). [default: 0]
        #[arg(short = 'L', long, default_value_t = 0)]
        ttl: i32,
    },
    /// Purge all volatile context for a session.
    Flush {
        /// Target session ID (omit for current).
        #[arg(short, long)]
        session: Option<String>,
        /// Explicit confirmation to bypass the safety gate.
        #[arg(long)]
        confirm: bool,
    },
    /// List available context quicksaves.
    List {
        /// Filter by agent name.
        #[arg(short, long)]
        agent: Option<String>,
    },
    /// Restore a session's hot context from a quicksave.
    Restore {
        /// Snapshot ID.
        id: String,
        /// Target session ID (omit for current).
        #[arg(short, long)]
        session: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum ConfigAction {
    /// Set a dynamic configuration value in Redis (Hot Config).
    Set { key: String, value: String },
    /// Get a specific configuration value.
    Get { key: String },
    /// List all extra configuration keys.
    List,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ImportRoute {
    GithubIssues,
    Hydration,
    Knowledge,
}

#[derive(Subcommand)]
pub enum IntelAction {
    /// Query the collective knowledge bank.
    Query {
        /// Search term or regex pattern.
        term: String,
        /// Maximum results to return.
        #[arg(short, long, default_value_t = 10)]
        limit: usize,
        /// Filter by specific tags.
        #[arg(short, long)]
        tags: Option<String>,
        /// Filter by agent identity (Captain's Oversight).
        #[arg(short, long)]
        agent: Option<String>,
    },

    /// Commit a fact or learning to the durable memory bank.
    Remember {
        #[command(subcommand)]
        category: MemoryCategory,
    },

    /// Record a persona-specific reflection or architectural thought.
    Ponder {
        /// The reflection content.
        text: String,
        /// Optional classification tags.
        #[arg(short, long)]
        tags: Option<String>,
    },

    /// Access the KoadOS Field Guide for a specific topic.
    Guide { topic: Option<String> },

    /// Perform a deep recursive scan of the workspace for project roots.
    Scan { path: Option<PathBuf> },

    /// Introspect on cognitive health and learning status.
    Mind {
        #[command(subcommand)]
        action: MindAction,
    },

    /// Retrieve a precise line-range snippet from a file.
    Snippet {
        /// Target file path.
        path: PathBuf,
        /// Start line (1-indexed).
        #[arg(short, long)]
        start: i32,
        /// End line (inclusive).
        #[arg(short, long)]
        end: i32,
        /// Force reload from disk.
        #[arg(short, long)]
        bypass: bool,
    },
}

#[derive(Subcommand)]
pub enum MemoryCategory {
    /// A persistent system fact.
    Fact {
        text: String,
        #[arg(short, long)]
        tags: Option<String>,
    },
    /// A technical insight or discovery.
    Learning {
        text: String,
        #[arg(short, long)]
        tags: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum FleetAction {
    /// Manage the high-level Command Deck.
    Board {
        #[command(subcommand)]
        action: BoardAction,
    },
    /// Low-level project mapping.
    Project {
        #[command(subcommand)]
        action: ProjectAction,
    },
    /// Atomic task tracking and state transitions.
    Issue {
        #[command(subcommand)]
        action: IssueAction,
    },
}

#[derive(Subcommand)]
pub enum BoardAction {
    /// Display current project board items.
    Status {
        /// Only show 'In Progress' and 'Todo' items.
        #[arg(short, long)]
        active: bool,
    },
    /// Perform a 2-way sync between GitHub and the Local Memory Bank.
    Sync {
        #[arg(long)]
        dry_run: bool,
    },
    /// Transition a node to 'Done' on the Command Deck.
    Done {
        id: i32,
        /// Explicit confirmation to bypass the safety gate.
        #[arg(long)]
        confirm: bool,
    },
    /// Re-open a node or move to 'Todo'.
    Todo { id: i32 },
    /// Run a Strategic Design Review (SDR).
    Sdr,
    /// Verify a node's status against the Command Deck.
    Verify { id: i32 },
}

#[derive(Subcommand)]
pub enum ProjectAction {
    /// List all registered projects in the Master Map.
    List,
    /// Manually register a new project root.
    Register {
        /// Project identifier.
        name: String,
        /// Physical directory path.
        path: Option<PathBuf>,
    },
    /// Update project health and branch metadata.
    Sync { id: Option<i32> },
    /// Display detailed project diagnostics.
    Info { id: i32 },
    /// Mark a project as retired or inactive.
    Retire { id: i32 },
}

#[derive(Subcommand)]
pub enum IssueAction {
    /// Track an existing GitHub issue in the local task graph.
    Track { number: i32, description: String },
    /// Advance an issue through the KoadOS Canon steps (1-9).
    Move { number: i32, step: i32 },
    /// Authorize implementation or closure (Admin/Captain only).
    Approve { number: i32 },
    /// Close an issue locally and on GitHub.
    Close { number: i32 },
    /// Show detailed sovereignty status for an issue.
    Status { number: i32 },
}

#[derive(Subcommand)]
pub enum BridgeAction {
    /// Interface with Google Cloud Platform.
    Gcloud,
    /// Synchronize data with Airtable.
    Airtable,
    /// Interface with Notion (Optimized Native Bridge).
    Notion {
        #[command(subcommand)]
        action: NotionAction,
    },
    /// Interface with local Filesystem (Scoped MCP).
    Fs {
        #[command(subcommand)]
        action: FsAction,
    },
    /// Execute a global cloud-to-local sync.
    Sync,
    /// Manage Google Drive file anchors.
    Drive,
    /// Post a high-priority event to the KoadStream.
    Stream {
        #[command(subcommand)]
        action: StreamAction,
    },
    /// Manage and execute specialized KoadOS Skills.
    Skill {
        #[command(subcommand)]
        action: SkillAction,
    },
    /// Publish local changes to the remote grid (Git Push).
    Publish {
        /// Commit message.
        #[arg(short, long)]
        message: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum NotionAction {
    /// Read a page's content as surgically parsed Markdown.
    Read {
        /// The Notion Page ID.
        id: String,
    },
    /// Synchronize all pages from a Notion database to local SQLite.
    Sync {
        /// The Notion Database ID.
        id: String,
    },
    /// Export synced pages from SQLite to local Markdown files.
    Export {
        /// The Notion Database ID.
        id: String,
        /// The output directory for Markdown files.
        #[arg(short, long)]
        output: PathBuf,
    },
    /// Update the status of a Notion page.
    UpdateStatus {
        /// The Notion Page ID.
        id: String,
        /// New Status Name.
        status: String,
    },
    /// Post a high-priority message to the KoadStream.
    Stream {
        /// The message to post.
        message: String,
        /// Target agent (e.g., Sky, Clyde, Noti). [default: Tyr]
        #[arg(short, long, default_value = "Tyr")]
        target: String,
        /// Priority level (Low, Medium, High). [default: Medium]
        #[arg(short, long, default_value = "Medium")]
        priority: String,
    },
}

#[derive(Subcommand)]
pub enum FsAction {
    /// Start the scoped Filesystem MCP Server (Stdio).
    Serve,
}

#[derive(Subcommand)]
pub enum StreamAction {
    /// Broadcast a message to the Neural Bus.
    Post {
        /// Topic or source identifier.
        topic: String,
        /// Event payload.
        message: String,
        /// Severity level (INFO, WARN, ERROR, CRITICAL).
        #[arg(short, long, default_value = "INFO")]
        msg_type: String,
    },
}

#[derive(Subcommand)]
pub enum SkillAction {
    /// List all currently registered Skills.
    List,
    /// Register a WASM-based Skill with the CASS ToolRegistry.
    Register {
        /// Skill name.
        name: String,
        /// Absolute path to the .wasm component file.
        path: String,
    },
    /// Deregister a Skill from the CASS ToolRegistry.
    Deregister {
        /// Skill name.
        name: String,
    },
    /// Invoke a registered Skill by name.
    Run {
        /// Skill name.
        name: String,
        /// Topic/action to invoke.
        topic: String,
        /// JSON payload for the skill.
        #[arg(default_value = "{}")]
        payload: String,
    },
}

#[derive(Subcommand)]
pub enum MindAction {
    /// Display cognitive health and learning metrics.
    Status,
    /// Capture a manual identity snapshot.
    Snapshot,
    /// Integrate a new structured insight into the Mind.
    Learn {
        /// Technical domain (e.g., rust, ops, architecture).
        domain: String,
        /// High-level summary of the insight.
        summary: String,
        /// Detailed technical breakdown.
        #[arg(short, long)]
        detail: Option<String>,
    },
}

#[derive(Subcommand, Debug)]
pub enum UpdatesAction {
    /// Post a new entry to the updates board.
    Post {
        /// One-line summary — the hydration atom. Keep under 120 chars.
        #[arg(short, long)]
        summary: String,
        /// Category: feature | fix | refactor | ops | identity | docs | infra [default: ops]
        #[arg(short, long, default_value = "ops")]
        category: String,
        /// Optional extended body in markdown.
        #[arg(short, long)]
        body: Option<String>,
        /// Board level: citadel | station | outpost (auto-detected from CWD if omitted).
        #[arg(short, long)]
        level: Option<String>,
        /// Override author name (defaults to $KOAD_AGENT_NAME).
        #[arg(short, long)]
        author: Option<String>,
    },
    /// Show recent entries in reverse-chronological order.
    List {
        /// Maximum entries to show. [default: 20]
        #[arg(short = 'n', long, default_value_t = 20)]
        limit: usize,
        /// Filter by author name.
        #[arg(short, long)]
        author: Option<String>,
        /// Filter by category.
        #[arg(short, long)]
        category: Option<String>,
        /// Board level (auto-detected from CWD if omitted).
        #[arg(short, long)]
        level: Option<String>,
    },
    /// Show the full detail of a specific entry.
    Show {
        /// Entry ID or partial filename to match.
        id: String,
    },
    /// Output a compact markdown digest for CASS context hydration.
    Digest {
        /// Maximum entries to include. [default: 10]
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
        /// Board level (auto-detected from CWD if omitted).
        #[arg(short, long)]
        level: Option<String>,
    },
}

#[derive(Subcommand)]
pub enum SignalAction {
    /// Send a signal to another agent.
    Send {
        /// Target agent name.
        target: String,
        /// Message content.
        #[arg(short, long)]
        message: String,
        /// Priority (low, standard, high, critical).
        #[arg(short, long, default_value = "standard")]
        priority: String,
    },
    /// List pending signals for the current agent.
    List {
        /// Show all signals including read and archived.
        #[arg(short, long)]
        all: bool,
    },
    /// Read a specific signal.
    Read {
        /// Signal ID.
        id: String,
    },
    /// Archive a signal.
    Archive {
        /// Signal ID.
        id: String,
    },
}
