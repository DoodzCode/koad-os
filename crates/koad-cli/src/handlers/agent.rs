//! # KoadOS Agent Identity Management
//!
//! Implements `koad agent new` and related subcommands for creating and inspecting
//! KoadOS Agent Identities (KAI) and their associated KAPV vaults.
//!
//! ## What `koad agent new` does
//!
//! Creating a new KAI touches every layer of the KoadOS identity system:
//!
//! ### Tier 1 — Machine-Required
//! 1. `config/identities/<key>.toml` — Primary identity record consumed by KoadConfig.
//!    Drives all boot logic: vault lookup, runtime enforcement, env injection,
//!    and Citadel bay auto-provisioning on kernel startup.
//! 2. `agents/<key>/` KAPV directory tree — The agent's persistent vault.
//!    `verify_kapv()` in koad-agent auto-creates the 7 standard dirs on first boot,
//!    but content files must be seeded here so the ghost has context from day one.
//!
//! ### KAPV Seeded Files
//! - `README.md`                  — Vault purpose and layout overview
//! - `AGENTS.md`                  — Agent-specific system prompt / identity lock
//! - `config/IDENTITY.toml`       — Local structured identity mirror
//! - `identity/IDENTITY.md`       — Concise identity anchor (boot step 3)
//! - `identity/XP_LEDGER.md`      — XP tracking table
//! - `instructions/RULES.md`      — Hard operating constraints
//! - `instructions/GUIDES.md`     — Boot sequence + working pattern guidance
//! - `memory/WORKING_MEMORY.md`   — Current session context (seeded empty)
//! - `memory/FACTS.md`            — Stable local facts (seeded with env facts)
//! - `memory/LEARNINGS.md`        — Durable lessons (seeded empty)
//! - `memory/SAVEUPS.md`          — Checkpoint log (seeded with creation entry)
//!
//! ### Tier 2 — Crew Docs (human-read at boot; affects agent context quality)
//! 3. `agents/CREW.md`   — Append row to personnel manifest table
//! 4. `AGENTS.md` (root)  — Append row to Section VII Personnel & Roles table
//!
//! ### Tier 3 — Automatic (no action required)
//! - Citadel bay store (`agents/bays/<key>/state.db`) — Auto-provisioned by kernel on startup
//!   via `BayStore::auto_provision_all()` which scans `config/identities/*.toml`.

use anyhow::{bail, Context, Result};
use koad_core::config::KoadConfig;
use std::path::{Path, PathBuf};
use tokio::fs;

use crate::cli::AgentAction;

pub async fn handle_agent_action(action: AgentAction, config: &KoadConfig) -> Result<()> {
    match action {
        AgentAction::New {
            name,
            rank,
            role,
            bio,
            runtime,
            vault,
            access_keys,
            tier,
            dry_run,
        } => {
            handle_new_agent(
                NewAgentRequest {
                    name: &name,
                    rank: &rank,
                    role: role.as_deref(),
                    bio: bio.as_deref(),
                    runtime: &runtime,
                    vault_override: vault.as_deref(),
                    access_keys_csv: &access_keys,
                    tier,
                    dry_run,
                },
                config,
            )
            .await
        }
        AgentAction::List => handle_list_agents(config).await,
        AgentAction::Info { agent } => handle_agent_info(&agent, config).await,
        AgentAction::Verify { agent } => handle_agent_verify(&agent, config).await,
        AgentAction::Prep { agent } => handle_agent_prep(&agent, config).await,
    }
}

// ---------------------------------------------------------------------------
// koad agent new
// ---------------------------------------------------------------------------

struct NewAgentRequest<'a> {
    name: &'a str,
    rank: &'a str,
    role: Option<&'a str>,
    bio: Option<&'a str>,
    runtime: &'a str,
    vault_override: Option<&'a str>,
    access_keys_csv: &'a str,
    tier: u32,
    dry_run: bool,
}

struct IdentityTomlSpec<'a> {
    key: &'a str,
    name: &'a str,
    role: &'a str,
    rank: &'a str,
    bio: &'a str,
    runtime: &'a str,
    vault_str: &'a str,
    access_keys_toml: &'a str,
    tier: u32,
}

struct KapvScaffoldSpec<'a> {
    vault: &'a Path,
    name: &'a str,
    key: &'a str,
    rank: &'a str,
    role: &'a str,
    bio: &'a str,
    runtime: &'a str,
    access_keys: &'a [String],
    tier: u32,
    today: &'a str,
}

async fn handle_new_agent(request: NewAgentRequest<'_>, config: &KoadConfig) -> Result<()> {
    let key = request.name.to_lowercase();
    let today = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let home_dir = dirs::home_dir().context("Could not determine home directory.")?;

    let identities_dir = config.home.join("config/identities");
    let identity_toml_path = identities_dir.join(format!("{}.toml", key));

    // =========================================================================
    // PATH A: TOML pre-exists — read identity from config, scaffold vault only
    // =========================================================================
    if identity_toml_path.exists() {
        let identity = config.identities.get(&key).with_context(|| {
            format!(
                "TOML exists at {} but could not be loaded — check for syntax errors.",
                identity_toml_path.display()
            )
        })?;

        let eff_role = identity.role.as_str();
        let eff_rank = identity.rank.as_str();
        let eff_bio = identity.bio.as_str();
        let eff_runtime = identity.runtime.as_deref().unwrap_or("gemini");
        let eff_tier = identity.tier;
        let eff_access_keys: Vec<String> = identity
            .preferences
            .as_ref()
            .map(|p| p.access_keys.clone())
            .unwrap_or_default();

        let vault_path = resolve_vault_path(
            request.vault_override,
            identity.vault.as_deref(),
            &key,
            &home_dir,
            config,
        );

        if vault_path.exists() {
            bail!(
                "Agent '{}' is already fully provisioned (TOML and vault both exist).\n  TOML:  {}\n  Vault: {}\nUse `koad agent verify {}` to check vault health.",
                request.name, identity_toml_path.display(), vault_path.display(), key
            );
        }

        println!(
            "\n\x1b[1;34m--- koad agent new: {} ---\x1b[0m{}",
            request.name,
            if request.dry_run {
                "  \x1b[33m[DRY RUN]\x1b[0m"
            } else {
                ""
            }
        );
        println!("  Key:     {}", key);
        println!("  Rank:    {}", eff_rank);
        println!("  Runtime: {}", eff_runtime);
        println!("  Vault:   {}", vault_path.display());
        println!(
            "\x1b[33m[INFO]\x1b[0m Existing TOML found — scaffolding vault and crew docs only."
        );
        println!();

        if request.dry_run {
            println!("\x1b[33m[DRY RUN]\x1b[0m Would create:");
            println!("  {}/  (KAPV tree)", vault_path.display());
            println!("  agents/CREW.md  (append row)");
            println!("  AGENTS.md        (append row)");
            return Ok(());
        }

        scaffold_kapv(
            KapvScaffoldSpec {
                vault: &vault_path,
                name: request.name,
                key: &key,
                rank: eff_rank,
                role: eff_role,
                bio: eff_bio,
                runtime: eff_runtime,
                access_keys: &eff_access_keys,
                tier: eff_tier,
                today: &today,
            },
            config,
        )
        .await?;
        patch_crew_md(config, request.name, eff_rank, eff_runtime, eff_role).await?;
        patch_root_agents_md(config, request.name, eff_rank, eff_role).await?;

        println!();
        println!(
            "\x1b[1;32m[OK]\x1b[0m Agent '{}' vault scaffolded from existing TOML. Boot with: \x1b[1meval $(KOAD_RUNTIME={} koad-agent boot {})\x1b[0m",
            request.name, eff_runtime, key
        );
        return Ok(());
    }

    // =========================================================================
    // PATH B: No TOML — require CLI args and create everything from scratch
    // =========================================================================
    let role = request.role.context(
        "--role is required when no identity TOML exists. \
         Either pass --role and --bio, or create config/identities/<key>.toml first.",
    )?;
    let bio = request.bio.context(
        "--bio is required when no identity TOML exists. \
         Either pass --role and --bio, or create config/identities/<key>.toml first.",
    )?;

    let vault_path = resolve_vault_path(request.vault_override, None, &key, &home_dir, config);
    let vault_str = vault_path.display().to_string();

    // Parse access keys
    let access_keys: Vec<String> = request
        .access_keys_csv
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let access_keys_toml = if access_keys.is_empty() {
        "access_keys = []".to_string()
    } else {
        let items = access_keys
            .iter()
            .map(|k| format!("\"{}\"", k))
            .collect::<Vec<_>>()
            .join(", ");
        format!("access_keys = [{}]", items)
    };

    println!(
        "\n\x1b[1;34m--- koad agent new: {} ---\x1b[0m{}",
        request.name,
        if request.dry_run {
            "  \x1b[33m[DRY RUN]\x1b[0m"
        } else {
            ""
        }
    );
    println!("  Key:     {}", key);
    println!("  Rank:    {}", request.rank);
    println!("  Runtime: {}", request.runtime);
    println!("  Vault:   {}", vault_path.display());
    println!();

    if request.dry_run {
        println!("\x1b[33m[DRY RUN]\x1b[0m Would create:");
        println!("  {}", identity_toml_path.display());
        println!("  {}/  (KAPV tree)", vault_path.display());
        println!("  agents/CREW.md  (append row)");
        println!("  AGENTS.md        (append row)");
        return Ok(());
    }

    // TIER 1-A: config/identities/<key>.toml
    let identity_toml = build_identity_toml(IdentityTomlSpec {
        key: &key,
        name: request.name,
        role,
        rank: request.rank,
        bio,
        runtime: request.runtime,
        vault_str: &vault_str,
        access_keys_toml: &access_keys_toml,
        tier: request.tier,
    });
    fs::create_dir_all(&identities_dir).await?;
    fs::write(&identity_toml_path, &identity_toml)
        .await
        .with_context(|| format!("Failed to write {}", identity_toml_path.display()))?;
    println!("\x1b[32m[CREATE]\x1b[0m {}", identity_toml_path.display());

    // TIER 1-B: KAPV vault scaffold
    scaffold_kapv(
        KapvScaffoldSpec {
            vault: &vault_path,
            name: request.name,
            key: &key,
            rank: request.rank,
            role,
            bio,
            runtime: request.runtime,
            access_keys: &access_keys,
            tier: request.tier,
            today: &today,
        },
        config,
    )
    .await?;

    // TIER 2: Crew docs
    patch_crew_md(config, request.name, request.rank, request.runtime, role).await?;
    patch_root_agents_md(config, request.name, request.rank, role).await?;

    println!();
    println!(
        "\x1b[1;32m[OK]\x1b[0m Agent '{}' registered. Boot with: \x1b[1meval $(KOAD_RUNTIME={} koad-agent boot {})\x1b[0m",
        request.name, request.runtime, key
    );
    println!(
        "     Citadel will auto-provision bay at startup: agents/bays/{}/state.db",
        key
    );

    Ok(())
}

fn resolve_vault_path(
    vault_override: Option<&str>,
    toml_vault: Option<&str>,
    key: &str,
    home_dir: &std::path::Path,
    config: &KoadConfig,
) -> PathBuf {
    let expand = |v: &str| -> PathBuf {
        if v.starts_with('~') {
            PathBuf::from(v.replacen('~', &home_dir.to_string_lossy(), 1))
        } else {
            PathBuf::from(v)
        }
    };
    if let Some(v) = vault_override {
        return expand(v);
    }
    if let Some(v) = toml_vault {
        return expand(v);
    }
    config.agent_dir(key)
}

// ---------------------------------------------------------------------------
// Identity TOML builder
// ---------------------------------------------------------------------------

fn build_identity_toml(spec: IdentityTomlSpec<'_>) -> String {
    format!(
        r#"[identities.{key}]
name = "{name}"
role = "{role}"
rank = "{rank}"
tier = {tier}
xp = 0
bio = "{bio}"
vault = "{vault_str}"
bootstrap = "{vault_str}/identity/IDENTITY.md"
runtime = "{runtime}"

[identities.{key}.preferences]
{access_keys_toml}

[identities.{key}.session_policy]
mode = "proactive"
timeout_minutes = 240
auto_saveup = true
"#,
        key = spec.key,
        name = spec.name,
        role = spec.role,
        rank = spec.rank,
        tier = spec.tier,
        bio = spec.bio,
        vault_str = spec.vault_str,
        access_keys_toml = spec.access_keys_toml,
        runtime = spec.runtime,
    )
}

// ---------------------------------------------------------------------------
// KAPV scaffold
// ---------------------------------------------------------------------------

async fn scaffold_kapv(spec: KapvScaffoldSpec<'_>, config: &KoadConfig) -> Result<()> {
    // Create all standard directories (mirrors verify_kapv in koad-agent)
    let standard_dirs = [
        "bank",
        "config",
        "identity",
        "instructions",
        "memory",
        "reports",
        "sessions",
        "skills",
        "tasks",
        "templates",
    ];
    for d in &standard_dirs {
        fs::create_dir_all(spec.vault.join(d))
            .await
            .with_context(|| format!("Failed to create KAPV dir: {}", d))?;
    }

    let runtime_notes = match spec.runtime {
        "gemini" => "- This sanctuary supports Gemini CLI dark-mode operation.\n- `agent-boot` writes the generated anchor to `~/.gemini/GEMINI.md` — ephemeral.\n- This vault is the durable source of truth.",
        "codex" => "- Codex runs sandboxed. File writes outside `agents/<key>/` require user approval.\n- `agent-boot` writes the generated anchor to `~/.codex/AGENTS.md` — ephemeral.\n- This vault is the durable source of truth.",
        _ => "- This sanctuary's `AGENTS.md` is the identity lock for Claude Code sessions.\n- `agent-boot` writes the generated anchor to `~/.claude/CLAUDE.md` — ephemeral.\n- This vault is the durable source of truth.",
    };

    let koad_os_path = config.home.display().to_string();
    let home_dir = dirs::home_dir().context("Could not determine home directory.")?;
    let vault_path_str = spec.vault.display().to_string();

    // README.md
    write_file(
        spec.vault,
        "README.md",
        &format!(
            "# {name} KAPV\n\n{name} is a {rank} operating in a {runtime_body} body.\n\
         This directory is {name}'s personal KAPV (KoadOS Agent Personal Vault).\n\n\
         ## Purpose\n\n\
         Local source of truth for {name}'s sanctuary-level identity, working memory,\n\
         operating rules, and {runtime_body}-facing boot context.\n\n\
         ## Core Files\n\n\
         - `AGENTS.md` — identity lock for {runtime_body}\n\
         - `identity/IDENTITY.md` — concise identity anchor\n\
         - `config/IDENTITY.toml` — local structured identity mirror\n\
         - `instructions/RULES.md` — hard constraints\n\
         - `instructions/GUIDES.md` — boot and working guidance\n\
         - `memory/WORKING_MEMORY.md` — current session context\n\
         - `memory/FACTS.md` — stable local facts\n\
         - `memory/LEARNINGS.md` — durable lessons\n\
         - `memory/SAVEUPS.md` — checkpoint log\n\n\
         ## KAPV Layout\n\n\
         - `bank/` — local reference notes\n\
         - `config/` — sanctuary-local structured metadata\n\
         - `identity/` — role and ledger files\n\
         - `instructions/` — operating guidance\n\
         - `memory/` — durable personal memory\n\
         - `reports/` — investigations and audits\n\
         - `sessions/` — session-side notes\n\
         - `skills/` — agent skill documentation\n\
         - `tasks/` — local task records\n\
         - `templates/` — reusable templates\n",
            name = spec.name,
            rank = spec.rank,
            runtime_body = to_runtime_display(spec.runtime),
        ),
    )
    .await?;

    // AGENTS.md (identity lock / system prompt)
    write_file(spec.vault, "AGENTS.md", &format!(
        "# {name} — Agent Identity & Operating Protocols ({runtime_body})\n\n\
         **Role:** {role}\n\
         **Rank:** {rank}\n\n\
         **Status:** CONDITION GREEN (KAPV v1.0)\n\n\
         ---\n\n\
         ## I. Identity & Persona\n\n\
         - **Name:** {name}\n\
         - **Body:** {runtime_body} (Active)\n\
         - **Sanctuary:** `agents/{key}/` (Vault)\n\
         - **Runtime:** {runtime}\n\
         - **Tier:** {tier}\n\n\
         ---\n\n\
         ## II. Boot Protocol\n\n\
         1. **Hydrate & Anchor:** Run `agent-boot {key}` to inject identity and sync context.\n\
         2. **Read identity files** in order:\n\
            - `identity/IDENTITY.md`\n\
            - `instructions/RULES.md`\n\
            - `memory/WORKING_MEMORY.md`\n\n\
         ---\n\n\
         ## III. Non-Negotiable Directives\n\n\
         - **One Body, One Ghost:** One agent instance per session.\n\
         - **Sanctuary Rule:** Write authority scoped to `agents/{key}/` by default.\n\
           Operations outside this path require explicit Dood approval.\n\
         - **Dood Gate:** Architectural decisions require Condition Green before code runs.\n\
         - **No-Read Rule:** Never read entire files over 50 lines. Use grep and line-range reads.\n\
         - **Plan Mode Law:** Standard complexity tasks require a plan before execution.\n\n\
         ---\n\n\
         ## IV. Runtime Notes\n\n\
         {runtime_notes}\n",
        name = spec.name,
        key = spec.key,
        role = spec.role,
        rank = spec.rank,
        runtime = spec.runtime,
        runtime_body = to_runtime_display(spec.runtime),
        tier = spec.tier,
        runtime_notes = runtime_notes,
    )).await?;

    // config/IDENTITY.toml
    let ak_toml_list = if spec.access_keys.is_empty() {
        String::new()
    } else {
        format!(
            "\naccess_keys = [{}]",
            spec.access_keys
                .iter()
                .map(|k| format!("\"{}\"", k))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    write_file(
        spec.vault,
        "config/IDENTITY.toml",
        &format!(
            "[{key}]\n\
         name = \"{name}\"\n\
         title = \"{rank}\"\n\
         rank = \"{rank}\"\n\
         tier = {tier}\n\
         body = \"{runtime_body}\"\n\
         role = \"{role}\"\n\
         status = \"CONDITION GREEN\"\n\
         sanctuary = \"{vault_path}\"\n\
         source_of_truth = [\n\
         \x20 \"AGENTS.md\",\n\
         \x20 \"identity/IDENTITY.md\",\n\
         \x20 \"instructions/RULES.md\",\n\
         \x20 \"memory/WORKING_MEMORY.md\",\n\
         ]{ak_list}\n",
            key = spec.key,
            name = spec.name,
            rank = spec.rank,
            tier = spec.tier,
            role = spec.role,
            runtime_body = to_runtime_display(spec.runtime),
            vault_path = vault_path_str,
            ak_list = ak_toml_list,
        ),
    )
    .await?;

    // identity/IDENTITY.md
    write_file(
        spec.vault,
        "identity/IDENTITY.md",
        &format!(
            "# {name} Identity Anchor\n\n\
         - Name: `{name}`\n\
         - Rank: `{rank}`\n\
         - Tier: `{tier}`\n\
         - Body: `{runtime_body}`\n\
         - Role: `{role}`\n\
         - Sanctuary: `{vault_path}/`\n\
         - Bio: {bio}\n\n\
         *Established: {today}*\n",
            name = spec.name,
            rank = spec.rank,
            tier = spec.tier,
            runtime_body = to_runtime_display(spec.runtime),
            role = spec.role,
            bio = spec.bio,
            vault_path = vault_path_str,
            today = spec.today,
        ),
    )
    .await?;

    // identity/XP_LEDGER.md
    write_file(
        spec.vault,
        "identity/XP_LEDGER.md",
        &format!(
            "# {name} — XP Ledger\n\n\
         | Date | Task / Issue ID | Event | Delta | Running Total | Level |\n\
         | :--- | :--- | :--- | :--- | :--- | :--- |\n\
         | {today} | — | Opening Balance | +0 | 0 | Initiate (1) |\n",
            name = spec.name,
            today = spec.today,
        ),
    )
    .await?;

    // instructions/RULES.md
    write_file(
        spec.vault,
        "instructions/RULES.md",
        &format!(
            "# {name} Rules\n\n\
         ## Core\n\n\
         - One Body, One Ghost.\n\
         - This sanctuary is {name}'s private KAPV.\n\
         - Ghost persists across sessions — memory is half the agent.\n\n\
         ## Boundaries\n\n\
         - Local edits inside `{vault_path}/` are allowed without escalation.\n\
         - KoadOS source, shared config, or other agents' sanctuaries require Dood approval.\n\
         - Escalate architecture decisions to Tyr via GitHub issues.\n\n\
         ## Working Standard\n\n\
         - No-Read Rule: Never read full files over 50 lines.\n\
         - Plan Mode Law: Standard complexity tasks require a plan before code runs.\n\
         - All Rust code must pass `cargo clippy -- -D warnings`.\n\
         - Keep durable memory factual and minimal.\n\n\
         ## Saveup Protocol\n\n\
         - Log significant decisions to `memory/WORKING_MEMORY.md` during sessions.\n\
         - On session close, distill to `memory/LEARNINGS.md` and `memory/SAVEUPS.md`.\n\
         - XP events must be recorded in `identity/XP_LEDGER.md`.\n",
            name = spec.name,
            vault_path = vault_path_str,
        ),
    )
    .await?;

    // instructions/GUIDES.md
    write_file(
        spec.vault,
        "instructions/GUIDES.md",
        &format!(
            "# {name} Operating Guide\n\n\
         ## Boot Sequence\n\n\
         1. Run `eval $(koad-agent boot {key})` to hydrate identity.\n\
         2. Read `AGENTS.md`.\n\
         3. Read `identity/IDENTITY.md`.\n\
         4. Read `instructions/RULES.md`.\n\
         5. Read `memory/WORKING_MEMORY.md`.\n\n\
         ## Working Pattern\n\n\
         - Consult `koad map look` before any file traversal.\n\
         - Inspect local context before acting.\n\
         - Prefer precise edits over broad rewrites.\n\
         - Escalate KoadOS changes outside the vault to Dood/Tyr.\n\n\
         ## Task Protocol\n\n\
         1. **Research** — Gather context. Grep codebase, consult graph via `koad map`.\n\
         2. **Strategy** — Plan. Get Dood approval for Medium+ tasks.\n\
         3. **Execution** — Implement. Clippy-clean Rust. Targeted edits.\n\
         4. **KSRP** — Self-review. Log decisions to memory.\n",
            name = spec.name,
            key = spec.key,
        ),
    )
    .await?;

    // memory/WORKING_MEMORY.md
    write_file(
        spec.vault,
        "memory/WORKING_MEMORY.md",
        &format!(
            "# {name} — Working Memory\n\n\
         *Active session context. Updated during sessions, distilled at close.*\n\n\
         ## Current Status\n\n\
         - **Condition:** GREEN\n\
         - **Last Session:** {today} (Identity established)\n\n\
         ## Active Context\n\n\
         - Identity scaffolded and registered. First session pending.\n\n\
         ## Open Questions\n\n\
         - None.\n",
            name = spec.name,
            today = spec.today,
        ),
    )
    .await?;

    // memory/FACTS.md
    write_file(spec.vault, "memory/FACTS.md", &format!(
        "# {name} — Facts\n\n\
         *Stable, verified facts about the KoadOS environment.*\n\n\
         - KoadOS repo is at `{koad_os_path}` on the host environment.\n\
         - Platform identity is defined in `{koad_os_path}/config/identities/{key}.toml`.\n\
         - Vault is at `{koad_os_path}/agents/{key}/`.\n\
         - Bay DB will be at `{koad_os_path}/agents/bays/{key}/state.db` (auto-provisioned by Citadel).\n\
         - Dood (Ian) is the final approval gate for all architectural changes.\n",
        name = spec.name,
        key = spec.key,
        koad_os_path = koad_os_path,
    )).await?;

    // memory/LEARNINGS.md
    write_file(
        spec.vault,
        "memory/LEARNINGS.md",
        &format!(
            "# {name} — Learnings\n\n\
         *Durable lessons accumulated across sessions.*\n\n\
         | Date | Lesson |\n\
         | :--- | :--- |\n\
         | {today} | Identity established via `koad agent new`. |\n",
            name = spec.name,
            today = spec.today,
        ),
    )
    .await?;

    // memory/SAVEUPS.md
    write_file(
        spec.vault,
        "memory/SAVEUPS.md",
        &format!(
            "# {name} — Saveups\n\n\
         *Checkpoint log. One entry per significant session or milestone.*\n\n\
         ---\n\n\
         ## {today} — Identity Established\n\n\
         - **Event:** KAPV scaffolded via `koad agent new`.\n\
         - **Status:** CONDITION GREEN. Ready for first active session.\n",
            name = spec.name,
            today = spec.today,
        ),
    )
    .await?;

    println!(
        "\x1b[32m[CREATE]\x1b[0m {}/  ({} files seeded)",
        spec.vault.display(),
        standard_dirs.len() + 9
    );

    Ok(())
}

// ---------------------------------------------------------------------------
// Crew doc patching
// ---------------------------------------------------------------------------

/// Append a row to agents/CREW.md before the Dood (Admin) sentinel row.
async fn patch_crew_md(
    config: &KoadConfig,
    name: &str,
    rank: &str,
    runtime: &str,
    role: &str,
) -> Result<()> {
    let path = config.home.join("agents/CREW.md");
    let content = fs::read_to_string(&path)
        .await
        .with_context(|| format!("Could not read {}", path.display()))?;

    // Sentinel: Dood is always the last row before deployment protocols
    let sentinel = "| **Dood** |";
    if !content.contains(sentinel) {
        eprintln!(
            "\x1b[33m[WARN]\x1b[0m Could not find Dood sentinel in CREW.md — skipping patch."
        );
        return Ok(());
    }
    // Check for duplicate
    if content.contains(&format!("| **{}** |", name)) {
        println!(
            "\x1b[33m[SKIP]\x1b[0m {} already in CREW.md",
            path.display()
        );
        return Ok(());
    }

    let new_row = format!(
        "| **{}** | {} | {} | {}, Sovereign KAI | Research -> Strategy -> Execution (Dood Gate) |\n",
        name, rank, to_runtime_display(runtime), role
    );
    let patched = content.replacen(sentinel, &format!("{}{}", new_row, sentinel), 1);
    fs::write(&path, patched).await?;
    println!("\x1b[32m[PATCH]\x1b[0m {} (appended row)", path.display());
    Ok(())
}

/// Append a row to root AGENTS.md Section VII table before the Helm sentinel row.
async fn patch_root_agents_md(
    config: &KoadConfig,
    name: &str,
    rank: &str,
    role: &str,
) -> Result<()> {
    let path = config.home.join("AGENTS.md");
    let content = fs::read_to_string(&path)
        .await
        .with_context(|| format!("Could not read {}", path.display()))?;

    // Sentinel: Helm is typically last before the closing ---
    let sentinel = "| **Helm** |";
    if !content.contains(sentinel) {
        eprintln!("\x1b[33m[WARN]\x1b[0m Could not find Helm sentinel in root AGENTS.md — skipping patch.");
        return Ok(());
    }
    if content.contains(&format!("| **{}** |", name)) {
        println!(
            "\x1b[33m[SKIP]\x1b[0m {} already in root AGENTS.md",
            path.display()
        );
        return Ok(());
    }

    let focus = extract_focus(role);
    let new_row = format!("| **{}** | {} | {} | {} |\n", name, rank, role, focus);
    let patched = content.replacen(sentinel, &format!("{}{}", new_row, sentinel), 1);
    fs::write(&path, patched).await?;
    println!("\x1b[32m[PATCH]\x1b[0m {} (appended row)", path.display());
    Ok(())
}

// ---------------------------------------------------------------------------
// koad agent list / info / verify
// ---------------------------------------------------------------------------

async fn handle_list_agents(config: &KoadConfig) -> Result<()> {
    println!("\n\x1b[1;34m--- Registered KAI Identities ---\x1b[0m\n");
    println!(
        "  {:<12} {:<12} {:<10} {:<8}  Vault",
        "Name", "Rank", "Runtime", "XP"
    );
    println!("  {}", "-".repeat(70));

    if config.identities.is_empty() {
        println!("  (none)");
        return Ok(());
    }

    let mut entries: Vec<_> = config.identities.iter().collect();
    entries.sort_by_key(|(k, _)| k.as_str());

    for (key, id) in entries {
        let runtime = id.runtime.as_deref().unwrap_or("—");
        let vault = id.vault.as_deref().unwrap_or("(auto)");
        println!(
            "  {:<12} {:<12} {:<10} {:<8}  {}",
            id.name, id.rank, runtime, id.xp, vault
        );
        let _ = key; // suppress unused warning
    }
    println!();
    Ok(())
}

async fn handle_agent_info(agent: &str, config: &KoadConfig) -> Result<()> {
    let key = agent.to_lowercase();
    let id = config.identities.get(&key).with_context(|| {
        format!(
            "No identity found for '{}'. Check config/identities/{}.toml",
            agent, key
        )
    })?;

    println!("\n\x1b[1;34m--- Agent: {} ---\x1b[0m", id.name);
    println!("  Role:    {}", id.role);
    println!("  Rank:    {}", id.rank);
    println!("  XP:      {}", id.xp);
    println!("  Bio:     {}", id.bio);
    println!("  Runtime: {}", id.runtime.as_deref().unwrap_or("—"));
    println!("  Vault:   {}", id.vault.as_deref().unwrap_or("(auto)"));
    if let Some(prefs) = &id.preferences {
        if !prefs.access_keys.is_empty() {
            println!("  Keys:    {}", prefs.access_keys.join(", "));
        }
    }
    println!();
    Ok(())
}

async fn handle_agent_verify(agent: &str, config: &KoadConfig) -> Result<()> {
    let key = agent.to_lowercase();
    let home_dir = dirs::home_dir().context("Could not determine home directory.")?;

    let vault_path = if let Some(id) = config.identities.get(&key) {
        if let Some(v) = &id.vault {
            if v.starts_with('~') {
                PathBuf::from(v.replacen('~', &home_dir.to_string_lossy(), 1))
            } else {
                PathBuf::from(v)
            }
        } else {
            config.agent_dir(&key)
        }
    } else {
        config.agent_dir(&key)
    };

    if !vault_path.exists() {
        bail!(
            "Vault for '{}' not found at {}",
            agent,
            vault_path.display()
        );
    }

    let required_dirs = [
        "bank",
        "config",
        "identity",
        "instructions",
        "memory",
        "sessions",
        "tasks",
    ];
    let required_files = [
        "AGENTS.md",
        "identity/IDENTITY.md",
        "identity/XP_LEDGER.md",
        "instructions/RULES.md",
        "instructions/GUIDES.md",
        "memory/WORKING_MEMORY.md",
    ];

    let mut issues = 0;

    println!("\n\x1b[1;34m--- Verifying KAPV: {} ---\x1b[0m", agent);
    println!("  Vault: {}", vault_path.display());

    for d in &required_dirs {
        let p = vault_path.join(d);
        if p.exists() {
            println!("  \x1b[32m[OK]\x1b[0m  {}/", d);
        } else {
            println!("  \x1b[33m[HEAL]\x1b[0m {}/  (creating)", d);
            fs::create_dir_all(&p).await?;
            issues += 1;
        }
    }

    for f in &required_files {
        let p = vault_path.join(f);
        if p.exists() {
            println!("  \x1b[32m[OK]\x1b[0m  {}", f);
        } else {
            println!(
                "  \x1b[31m[MISS]\x1b[0m {}  (not seeded — run `koad agent new` to re-scaffold)",
                f
            );
            issues += 1;
        }
    }

    if issues == 0 {
        println!("\n  \x1b[32mKAPV is healthy.\x1b[0m");
    } else {
        println!("\n  \x1b[33m{} issue(s) found/healed.\x1b[0m", issues);
    }
    println!();
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

async fn write_file(vault: &Path, relative: &str, content: &str) -> Result<()> {
    let path = vault.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).await?;
    }
    fs::write(&path, content)
        .await
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(())
}

fn to_runtime_display(runtime: &str) -> &str {
    match runtime {
        "claude" => "Claude Code",
        "gemini" => "Gemini CLI",
        "codex" => "Codex CLI",
        other => other,
    }
}

/// Extract a concise focus summary from a role string for the AGENTS.md table.
fn extract_focus(role: &str) -> String {
    // Take first ~40 chars, stopping at a natural boundary
    if role.len() <= 40 {
        return role.to_string();
    }
    let truncated = &role[..40];
    match truncated.rfind([',', ' ', '-']) {
        Some(i) => format!("{}...", &truncated[..i]),
        None => format!("{}...", truncated),
    }
}

async fn handle_agent_prep(agent: &str, config: &KoadConfig) -> Result<()> {
    let key = agent.to_lowercase();
    let id = config.identities.get(&key).with_context(|| {
        format!(
            "No identity found for '{}'. Check config/identities/{}.toml",
            agent, key
        )
    })?;

    // Output to stderr to avoid polluting stdout (stdout is captured by eval)
    eprintln!("\n\x1b[1;34m--- KoadOS Agent Prep: {} ---\x1b[0m", id.name);
    eprintln!("  Role:    {}", id.role);
    eprintln!("  Rank:    {}", id.rank);
    eprintln!("  XP:      {}", id.xp);

    let home_dir = dirs::home_dir().context("Could not determine home directory.")?;
    let vault_path = if let Some(v) = &id.vault {
        if v.starts_with('~') {
            v.replacen('~', &home_dir.to_string_lossy(), 1)
        } else {
            v.clone()
        }
    } else {
        config.agent_dir(&key).to_string_lossy().into_owned()
    };

    let bootstrap_path = id.bootstrap.as_ref().map(|b| {
        if b.starts_with('~') {
            b.replacen('~', &home_dir.to_string_lossy(), 1)
        } else {
            b.clone()
        }
    });

    // Output env variables directly to stdout for shell to eval
    println!("export KOAD_AGENT_NAME=\"{}\";", id.name);
    println!("export KOAD_AGENT_RANK=\"{}\";", id.rank);
    println!("export KOAD_AGENT_ROLE=\"{}\";", id.role);
    println!("export KOAD_AGENT_BIO=\"{}\";", id.bio);
    println!("export KOAD_AGENT_XP=\"{}\";", id.xp);
    println!("export KOAD_VAULT_PATH=\"{}\";", vault_path);

    if let Some(ref run) = id.runtime {
        println!("export KOAD_RUNTIME=\"{}\";", run);
    }
    if let Some(ref bs) = bootstrap_path {
        println!("export KOAD_BOOTSTRAP=\"{}\";", bs);
    }

    // Resolve and export access keys
    if let Some(prefs) = &id.preferences {
        if !prefs.access_keys.is_empty() {
            eprintln!("  Resolving access keys:");
            for key_id in &prefs.access_keys {
                let resolved = config.resolve_secret(key_id, None);
                if !resolved.is_empty() {
                    println!("export {}={:?};", key_id, resolved);
                    eprintln!("      \x1b[32m✓\x1b[0m {}", key_id);
                } else {
                    eprintln!("      \x1b[33m⚠\x1b[0m {} (not resolved)", key_id);
                }
            }
        }
    }

    // HISTFILE isolation
    let session_dir = PathBuf::from(&vault_path).join("sessions");
    if session_dir.exists() {
        println!("export HISTFILE=\"{}/bash_history\";", session_dir.to_string_lossy());
        eprintln!("  Histfile: {}/bash_history", session_dir.to_string_lossy());
    }

    // Service status check
    eprintln!("  Checking Citadel pulse:");
    let local_koad_home = std::env::var("KOADOS_HOME")
        .or_else(|_| std::env::var("KOAD_HOME"))
        .unwrap_or_else(|_| format!("{}/.citadel-jupiter", home_dir.to_string_lossy()));
    let kh_path = PathBuf::from(local_koad_home);

    let socket_1 = kh_path.join("run/koad.sock");
    let socket_2 = kh_path.join("koad.sock");
    let redis_pass = socket_1.exists() || socket_2.exists();
    
    let redis_ok = redis_pass || std::net::TcpStream::connect("127.0.0.1:6379").is_ok();
    let grpc_ok = std::net::TcpStream::connect("127.0.0.1:50051").is_ok();
    let cass_ok = std::net::TcpStream::connect("127.0.0.1:50052").is_ok();

    if redis_ok {
        eprintln!("      Redis  (UDS/TCP):  \x1b[32m[PASS]\x1b[0m");
    } else {
        eprintln!("      Redis  (UDS/TCP):  \x1b[31m[FAIL]\x1b[0m");
    }

    if grpc_ok {
        eprintln!("      Citadel gRPC:      \x1b[32m[PASS]\x1b[0m");
    } else {
        eprintln!("      Citadel gRPC:      \x1b[33m[WARN]\x1b[0m (dark mode)");
    }

    if cass_ok {
        eprintln!("      CASS    gRPC:      \x1b[32m[PASS]\x1b[0m");
    } else {
        eprintln!("      CASS    gRPC:      \x1b[33m[WARN]\x1b[0m (dark mode)");
    }

    eprintln!("--------------------------------------------------");
    eprintln!("  Shell ready. Launch CLI or run: \x1b[1magent-boot {}\x1b[0m", id.name);
    eprintln!("--------------------------------------------------");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_focus_short_role_unchanged() {
        assert_eq!(extract_focus("Engineer"), "Engineer");
        assert_eq!(
            extract_focus("Citadel Build Engineer"),
            "Citadel Build Engineer"
        );
    }

    #[test]
    fn test_extract_focus_long_role_truncates_at_boundary() {
        let role = "Citadel Build Engineer, Container Operations, Execution Sandbox Oversight";
        let result = extract_focus(role);
        assert!(result.ends_with("..."), "should end with ellipsis");
        assert!(result.len() <= 43, "should not exceed 40 chars + ellipsis");
    }

    #[test]
    fn test_extract_focus_long_role_no_boundary_truncates_hard() {
        let role = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA"; // 46 chars, no boundary
        let result = extract_focus(role);
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_extract_focus_exact_boundary_length() {
        let role = "1234567890123456789012345678901234567890"; // exactly 40 chars
        assert_eq!(extract_focus(role), role); // no truncation at exact limit
    }
}
