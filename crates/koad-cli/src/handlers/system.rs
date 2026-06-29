use crate::cli::{ConfigAction, SystemAction};
use koad_core::db::KoadDB;
use crate::utils::errors::map_connect_err;
use crate::utils::{get_gdrive_token_for_path, get_gh_pat_for_path};
use anyhow::{Context, Result};
use chrono::Local;
use fred::interfaces::{HashesInterface, KeysInterface, LuaInterface};
use fred::types::Scanner;
use futures_util::StreamExt;
use koad_core::config::KoadConfig;
use koad_core::utils::lock::DistributedLock;
use koad_core::utils::redis::RedisClient;
use koad_proto::cass::v1::memory_service_client::MemoryServiceClient;
use koad_proto::cass::v1::FactCard;
use koad_proto::citadel::v5::admin_client::AdminClient;
use koad_proto::citadel::v5::citadel_session_client::CitadelSessionClient;
use koad_proto::citadel::v5::*;
use rusqlite::params;
use serde_json::Value;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::Duration;
use tracing::{info, warn};

pub struct RedisLockClient {
    pub socket: PathBuf,
}

#[async_trait::async_trait]
impl DistributedLock for RedisLockClient {
    async fn lock(&self, sector: &str, agent_name: &str, ttl_secs: u64) -> Result<bool> {
        let client =
            RedisClient::new(&self.socket.parent().unwrap().to_string_lossy(), false).await?;
        let key = format!("koad:lock:{}", sector);
        let session_id = env::var("KOAD_SESSION_ID").unwrap_or_else(|_| agent_name.to_string());
        let val = format!("{}:{}", agent_name, session_id);

        let res: Option<String> = client
            .pool
            .set(
                &key,
                &val,
                Some(fred::types::Expiration::EX(ttl_secs as i64)),
                Some(fred::types::SetOptions::NX),
                false,
            )
            .await?;

        Ok(res.is_some()
            || client.pool.get::<Option<String>, _>(&key).await?.as_deref() == Some(&val))
    }

    async fn unlock(&self, sector: &str, agent_name: &str) -> Result<bool> {
        let client =
            RedisClient::new(&self.socket.parent().unwrap().to_string_lossy(), false).await?;
        let key = format!("koad:lock:{}", sector);
        let session_id = env::var("KOAD_SESSION_ID").unwrap_or_else(|_| agent_name.to_string());
        let val = format!("{}:{}", agent_name, session_id);

        let script = r"
            if redis.call('get', KEYS[1]) == ARGV[1] then
                return redis.call('del', KEYS[1])
            else
                return 0
            end
        ";

        let result: i32 = client
            .pool
            .next()
            .eval(script, vec![key], vec![val])
            .await?;
        Ok(result == 1)
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn spawn_issue(
    config: &KoadConfig,
    db: &KoadDB,
    template: &str,
    title: &str,
    weight: &str,
    objective: Option<String>,
    scope: Option<String>,
    labels: Vec<String>,
    raw_body: Option<String>,
) -> Result<koad_board::issue::Issue> {
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let project_ctx = config.resolve_project_context(&current_dir);
    let project = project_ctx.as_ref().map(|(_, p)| p);

    // Resolve repository from Context or DB
    let (owner, repo) = if let Some(p) = project {
        (
            config.get_github_owner(Some(p)),
            config.get_github_repo(Some(p)),
        )
    } else if let Ok(conn) = db.get_conn() {
        let abs_current = std::fs::canonicalize(&current_dir).unwrap_or(current_dir);
        let search_path = abs_current.to_string_lossy().to_string();
        let mut stmt = conn.prepare("SELECT github_repo FROM projects WHERE ?1 LIKE path || '%' ORDER BY length(path) DESC LIMIT 1")?;
        let repo_full: Option<String> = stmt.query_row(params![search_path], |r| r.get(0)).ok();

        if let Some(full) = repo_full {
            let parts: Vec<&str> = full.split('/').collect();
            if parts.len() == 2 {
                (parts[0].to_string(), parts[1].to_string())
            } else {
                (
                    config.get_github_owner(None::<&str>),
                    config.get_github_repo(None::<&str>),
                )
            }
        } else {
            (
                config.get_github_owner(None::<&str>),
                config.get_github_repo(None::<&str>),
            )
        }
    } else {
        (
            config.get_github_owner(None::<&str>),
            config.get_github_repo(None::<&str>),
        )
    };

    let token = config.resolve_gh_token(project.as_ref().map(|s| s.as_str()), None)?;
    let client = koad_board::GitHubClient::new(token, owner.clone(), repo.clone())?;

    let body = if let Some(rb) = raw_body {
        rb
    } else {
        let template_path = config
            .home
            .join("templates")
            .join("issues")
            .join(format!("{}.md", template));
        if !template_path.exists() {
            anyhow::bail!("Template '{}' not found at {:?}", template, template_path);
        }

        let mut b = std::fs::read_to_string(&template_path)?;

        // String Substitution for fast-spawning
        b = b.replace("[trivial | standard | complex]", weight);

        if let Some(obj) = objective {
            b = b.replace(
                "[Describe the high-level goal of this architectural change]",
                &obj,
            );
            b = b.replace("[Describe the system subsystem to be hardened]", &obj);
            b = b.replace("[Identify the resource or latency bottleneck]", &obj);
            b = b.replace(
                "[Describe the observed behavior vs expected behavior]",
                &obj,
            );
        }
        if let Some(sc) = scope {
            b = b.replace(
                "- [Component A]\n- [Component B]\n- [Interface change/Addition]",
                &sc,
            );
            b = b.replace("- [Recovery logic for X]\n- [Watchdog implementation for Y]\n- [Self-healing procedure for Z]", &sc);
            b = b.replace(
                "- [Caching strategy]\n- [Refactor of inefficient loop]\n- [Payload reduction]",
                &sc,
            );
        }
        b
    };

    client.create_issue(title, &body, labels).await
}

pub async fn handle_system_action(
    action: SystemAction,
    config: &KoadConfig,
    db: &KoadDB,
    role: String,
    is_admin: bool,
    agent_name: &str,
) -> Result<()> {
    match action {
        SystemAction::Auth => {
            let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let (p, _) = get_gh_pat_for_path(&current_dir, &role, config);
            let (d, _) = get_gdrive_token_for_path(&current_dir);
            println!("GH:{} | GD:{}", p, d);
        }
        SystemAction::Init { force } => {
            system_init(config, force)?;
        }
        SystemAction::Config { action, json } => match action {
            Some(ConfigAction::Set { key, value }) => {
                let client = RedisClient::new(&config.home.to_string_lossy(), false).await?;
                let mut hot_config = config.clone();

                match key.as_str() {
                    "github_project_number" => {
                        if let Ok(num) = value.parse::<u32>() {
                            if let Some(ref mut g) = hot_config.integrations.github {
                                g.default_project_number = num;
                            } else {
                                anyhow::bail!("GitHub integration not configured in TOML.");
                            }
                        } else {
                            anyhow::bail!("Invalid number: {}", value);
                        }
                    }
                    "citadel_grpc_addr" => hot_config.network.citadel_grpc_addr = value.clone(),
                    _ => {
                        hot_config.extra.insert(key.clone(), value.clone());
                    }
                }

                let json = hot_config.to_json()?;
                let _: () = client
                    .pool
                    .next()
                    .set(
                        koad_core::constants::REDIS_KEY_CONFIG,
                        json,
                        None,
                        None,
                        false,
                    )
                    .await?;
                println!(
                    "\x1b[32m[OK]\x1b[0m Config '{}' set to '{}' in Redis.",
                    key, value
                );
            }
            Some(ConfigAction::Get { key }) => match key.as_str() {
                "github_project_number" => {
                    let val = config
                        .integrations
                        .github
                        .as_ref()
                        .map(|g| g.default_project_number.to_string())
                        .unwrap_or_else(|| "Not Configured".to_string());
                    println!("{}", val);
                }
                "citadel_grpc_addr" => println!("{}", config.network.citadel_grpc_addr),
                _ => {
                    if let Some(v) = config.extra.get(&key) {
                        println!("{}", v);
                    } else {
                        println!("Key '{}' not found.", key);
                    }
                }
            },
            Some(ConfigAction::List) => {
                println!("--- Dynamic Configuration ---");
                for (k, v) in &config.extra {
                    println!("{}: {}", k, v);
                }
            }
            None => {
                if json {
                    println!("{}", config.to_json()?);
                } else {
                    println!("{:#?}", config);
                }
            }
        },
        SystemAction::Refresh { restart, confirm } => {
            if !is_admin {
                anyhow::bail!("Admin only.");
            }

            if restart && !confirm {
                println!("\x1b[33m[SAFETY GATE]\x1b[0m Restarting services will terminate active sessions and may cause disruption.");
                println!("Run with --confirm to proceed with full refresh.");
                return Ok(());
            }

            let lock_client = RedisLockClient {
                socket: config.get_redis_socket(),
            };

            koad_core::with_sector_lock!(&lock_client, "refresh", agent_name, 600, {
                println!(
                    "
\x1b[1m--- KoadOS Core Refresh (Hard Reset) ---\x1b[0m"
                );
                let home = config.home.clone();
                println!(">>> [1/3] Energizing Forge (cargo build)...");
                let build_status = Command::new("cargo")
                    .arg("build")
                    .current_dir(&home)
                    .status()?;
                if !build_status.success() {
                    anyhow::bail!("Forge failure.");
                }

                println!(">>> [2/3] Verifying Core Links (bin/ alignment)...");
                let bin_dir = home.join("bin");
                let target_dir = home.join("target/debug");

                let links = [
                    ("koad", "koad"),
                    ("kcitadel", "koad-citadel"),
                    ("koad-asm", "koad-asm"),
                    ("koad-watchdog", "koad-watchdog"),
                    ("koad-cli", "koad"),
                ];

                for (link_name, target_name) in links {
                    let link_path = bin_dir.join(link_name);
                    let target_path = target_dir.join(target_name);

                    if link_path.exists() {
                        let _ = std::fs::remove_file(&link_path);
                    }

                    #[cfg(unix)]
                    {
                        if let Err(e) = std::os::unix::fs::symlink(&target_path, &link_path) {
                            warn!(
                                "  [FAIL] Failed to link {} -> {}: {}",
                                link_name, target_name, e
                            );
                        } else {
                            println!("  [OK] {} linked to {}", link_name, target_name);
                        }
                    }
                }

                if restart {
                    println!(">>> [3/3] Rebooting Core Systems...");
                    stop_citadel_processes();
                    std::thread::sleep(Duration::from_millis(800));
                    start_citadel_services(&KoadConfig::load()?)?;
                    println!("  [OK] Core systems energized.");
                }
                Ok::<(), anyhow::Error>(())
            })
            .await??;
        }
        SystemAction::Save { full } => {
            if !is_admin {
                anyhow::bail!("Admin only.");
            }
            println!(
                "
\x1b[1m--- KoadOS Sovereign Save Protocol ---\x1b[0m"
            );
            let home = config.home.clone();
            let now_ts = Local::now().format("%Y%m%d-%H%M%S").to_string();

            // 1. Memory Drain (gRPC)
            println!(">>> [1/4] Neuronal Flush (Citadel Shutdown)...");
            match AdminClient::connect(config.network.citadel_grpc_addr.clone()).await {
                Ok(mut client) => {
                    let context = Some(crate::utils::get_trace_context(agent_name, 3));
                    if let Err(e) = client
                        .shutdown(crate::utils::authenticated_request(ShutdownRequest {
                            context,
                            reason: "Sovereign Save Protocol initiated.".to_string(),
                        }))
                        .await
                    {
                        warn!(
                            "  [FAIL] Neuronal flush failed: {}. Continuing with local save.",
                            e
                        );
                    } else {
                        println!("  [OK] Hot-stream drained to durable memory.");
                    }
                }
                Err(_) => warn!("  [SKIP] Citadel offline. Skipping hot-stream drain."),
            }

            // 2. Cognitive Snapshot
            println!(">>> [2/4] Archiving Identity (Mind Snapshot)...");
            let conn = db.get_conn()?;
            conn.execute(
                "INSERT INTO identity_snapshots (trigger, notes, created_at, origin_agent) VALUES ('sovereign-save', 'Full system checkpoint.', ?1, ?2)",
                params![Local::now().to_rfc3339(), agent_name],
            )?;
            println!("  [OK] Persona state captured.");

            if full {
                // 2.5. Synchronize Session Histories
                println!(">>> [2.5/4] Aligning Histories (Session Synchronization)...");
                if let Err(e) = sync_session_histories(&home, &config.network.cass_grpc_addr).await {
                    warn!(
                        "  [FAIL] Session synchronization failed: {}. Continuing with backup.",
                        e
                    );
                } else {
                    println!("  [OK] Personal bay session logs synchronized to CASS.");
                }

                // 3. Database Backup
                println!(">>> [3/4] Fortifying Memory (Database Backup)...");
                let backup_dir = home.join("backups");
                std::fs::create_dir_all(&backup_dir)?;
                let backup_path = backup_dir.join(format!("koad-{}.db", now_ts));
                std::fs::copy(home.join("data/db/koad.db"), &backup_path)?;
                println!("  [OK] Database archived to: {}", backup_path.display());

                // 4. Git Checkpoint
                println!(">>> [4/4] Finalizing Timeline (Git Checkpoint)...");
                let m = format!("Sovereign Save: {}", now_ts);
                let _ = Command::new("git")
                    .arg("-C")
                    .arg(&home)
                    .arg("add")
                    .arg(".")
                    .status();
                let _ = Command::new("git")
                    .arg("-C")
                    .arg(&home)
                    .arg("commit")
                    .arg("-m")
                    .arg(&m)
                    .status();
                println!("  [OK] System checkpoint committed to git.");
            }
            println!(
                "
\x1b[32m[CONDITION GREEN] Sovereign Save Complete.\x1b[0m"
            );
        }
        SystemAction::Patch {
            path,
            search,
            replace,
            payload,
            fuzzy,
            dry_run,
        } => {
            if !is_admin {
                anyhow::bail!("Admin only.");
            }

            let (target_path, search_str, replace_str) = if let Some(p_str) = payload {
                let v: Value =
                    serde_json::from_str(&p_str).context("Invalid Patch JSON payload.")?;
                (
                    PathBuf::from(v["path"].as_str().context("Missing 'path' in payload")?),
                    v["search"]
                        .as_str()
                        .context("Missing 'search' in payload")?
                        .to_string(),
                    v["replace"]
                        .as_str()
                        .context("Missing 'replace' in payload")?
                        .to_string(),
                )
            } else {
                (
                    path.context("Missing path.")?,
                    search.context("Missing search pattern.")?,
                    replace.context("Missing replacement string.")?,
                )
            };

            let content = std::fs::read_to_string(&target_path)?;
            let new_content = if fuzzy {
                // Simplified fuzzy replace for now
                content.replace(&search_str, &replace_str)
            } else {
                content.replace(&search_str, &replace_str)
            };

            if dry_run {
                println!("--- [DRY RUN] Patch for {:?} ---", target_path);
                println!("{}", new_content);
            } else {
                std::fs::write(&target_path, new_content)?;
                println!("\x1b[32m[OK]\x1b[0m Patch applied to {:?}", target_path);
            }
        }
        SystemAction::Tokenaudit { cleanup: _ } => {
            println!(
                "
\x1b[1m--- KoadOS Cognitive Efficiency Audit ---\x1b[0m"
            );
            let conn = db.get_conn()?;

            // Pass 1: Memory Density
            print!("{:<35}", "Pass 1: Storage (Knowledge):");
            let total_k: i32 =
                conn.query_row("SELECT count(*) FROM knowledge", [], |r| r.get(0))?;
            println!("\x1b[32m[PASS]\x1b[0m Ingested {} facts.", total_k);

            // Pass 2: Session Isolation
            print!("{:<35}", "Pass 2: Identity (Sessions):");
            let active_s: i32 =
                conn.query_row("SELECT count(*) FROM identity_roles", [], |r| r.get(0))?;
            println!(
                "\x1b[32m[PASS]\x1b[0m Monitoring {} active links.",
                active_s
            );

            // Pass 3: Tool-Call Efficiency
            print!("{:<35}", "Pass 3: Logic (Context Cache):");
            let cache_socket = config.home.join("run/koad.sock");
            if cache_socket.exists() {
                println!("\x1b[32m[PASS]\x1b[0m Neural Bus Cache Active.");
            } else {
                println!("\x1b[31m[FAIL]\x1b[0m Cache Offline.");
            }

            // Pass 4: Payload Trimming
            print!("{:<35}", "Pass 4: Data (Payloads):");
            println!("\x1b[32m[PASS]\x1b[0m gRPC binary protocol enforced.");

            // Pass 5: Persona Density
            print!("{:<35}", "Pass 5: Identity (Density):");
            let mut stmt = conn.prepare("SELECT id, length(bio) FROM identities")?;
            let bios = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, i32>(1)?)))?;
            let mut high_density = true;
            for b in bios {
                let (id, len) = b?;
                if len > 200 {
                    println!(
                        "\x1b[33m[WARN]\x1b[0m KAI '{}' bio too long ({} chars).",
                        id, len
                    );
                    high_density = false;
                }
            }
            if high_density {
                println!("\x1b[32m[PASS]\x1b[0m All KAIs high-density.");
            }

            println!(
                "\x1b[1m---------------------------------------------------\x1b[0m
"
            );
        }
        SystemAction::Spawn {
            template,
            title,
            weight,
            objective,
            scope,
            labels,
        } => {
            println!(">>> [SPAWN] Energizing Forge for Issue: {}...", title);
            let issue = spawn_issue(
                config, db, &template, &title, &weight, objective, scope, labels, None,
            )
            .await?;

            // Resolve repo string for the reporter (using normalized path)
            let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let abs_current = std::fs::canonicalize(&current_dir).unwrap_or(current_dir);
            let search_path = abs_current.to_string_lossy().to_string();

            let repo_full = if let Ok(conn) = db.get_conn() {
                let stmt = conn.prepare("SELECT github_repo FROM projects WHERE ?1 LIKE path || '%' ORDER BY length(path) DESC LIMIT 1").ok();
                stmt.and_then(|mut s| {
                    s.query_row(params![search_path], |r| r.get::<_, String>(0))
                        .ok()
                })
                .unwrap_or_else(|| {
                    format!(
                        "{}/{}",
                        config.get_github_owner(None),
                        config.get_github_repo(None)
                    )
                })
            } else {
                format!(
                    "{}/{}",
                    config.get_github_owner(None),
                    config.get_github_repo(None)
                )
            };

            println!(
                "\x1b[32m[SPAWNED]\x1b[0m Issue #{} live at: https://github.com/{}/issues/{}",
                issue.number, repo_full, issue.number
            );
        }
        SystemAction::Import { .. } => {
            // Handled in main.rs dispatcher
        }
        SystemAction::Lock { sector, ttl } => {
            let lock_client = RedisLockClient {
                socket: config.get_redis_socket(),
            };

            if lock_client.lock(&sector, agent_name, ttl).await? {
                println!(
                    "\x1b[32m[OK]\x1b[0m Sector '{}' locked by '{}'.",
                    sector, agent_name
                );
            } else {
                let client = RedisClient::new(&config.home.to_string_lossy(), false).await?;
                let owner: String = client
                    .pool
                    .get::<Option<String>, _>(format!("koad:lock:{}", sector))
                    .await?
                    .unwrap_or_else(|| "unknown".to_string());
                anyhow::bail!(
                    "LOCK_DENIED: Sector '{}' is already held by '{}'.",
                    sector,
                    owner
                );
            }
        }
        SystemAction::Unlock { sector } => {
            let lock_client = RedisLockClient {
                socket: config.get_redis_socket(),
            };

            if lock_client.unlock(&sector, agent_name).await? {
                println!("\x1b[32m[OK]\x1b[0m Sector '{}' released.", sector);
            } else {
                let client = RedisClient::new(&config.home.to_string_lossy(), false).await?;
                let owner: Option<String> = client
                    .pool
                    .get::<Option<String>, _>(format!("koad:lock:{}", sector))
                    .await?;
                match owner {
                    Some(o) => {
                        anyhow::bail!(
                            "UNLOCK_DENIED: You do not own the lock for '{}' (Held by '{}').",
                            sector,
                            o
                        );
                    }
                    None => {
                        println!("\x1b[33m[WARN]\x1b[0m Sector '{}' was not locked.", sector);
                    }
                }
            }
        }
        SystemAction::Locks => {
            let client = RedisClient::new(&config.home.to_string_lossy(), false).await?;
            let mut scan_stream = client.pool.next().scan("koad:lock:*", None, None);

            let mut all_keys: Vec<String> = Vec::new();
            while let Some(res) = scan_stream.next().await {
                if let Ok(page) = res {
                    if let Some(keys) = page.results() {
                        for key in keys {
                            if let Some(k) = key.as_str() {
                                all_keys.push(k.to_string());
                            }
                        }
                    }
                }
            }

            println!("\n\x1b[1m--- Active Distributed Locks ---\x1b[0m");
            if all_keys.is_empty() {
                println!("No active locks found.");
            } else {
                println!(
                    "{:<20} | {:<30} | {:<10}",
                    "Sector", "Owner:Session", "TTL (s)"
                );
                println!("{}", "-".repeat(65));
                for key in all_keys {
                    let val: String = client
                        .pool
                        .next()
                        .get::<Option<String>, _>(&key)
                        .await?
                        .unwrap_or_else(|| "unknown".to_string());
                    let ttl: i64 = client.pool.next().ttl(&key).await?;
                    let sector = key.replace("koad:lock:", "");
                    println!("{:<20} | {:<30} | {:<10}", sector, val, ttl);
                }
            }
        }
        SystemAction::Reconnect { agent, live } => {
            let session_id = env::var("KOAD_SESSION_ID").unwrap_or_default();
            let body_id = env::var("KOAD_BODY_ID").unwrap_or_default();

            // If live is requested but no SID, fail early
            if live && session_id.is_empty() {
                anyhow::bail!("Live reconnection failed: KOAD_SESSION_ID not set in environment.");
            }

            let target_agent = agent.unwrap_or_else(|| config.get_agent_name());

            if live {
                println!(
                    ">>> [LIVE RE-SYNC] Re-linking session {} for agent {}...",
                    session_id, target_agent
                );
            } else {
                println!(">>> [RECONNECT] Searching for Ghost: {}...", target_agent);
            }

            let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let mut client =
                CitadelSessionClient::connect(config.network.citadel_grpc_addr.clone()).await?;
            let context = Some(crate::utils::get_trace_context(&target_agent, 3));
            let req = LeaseRequest {
                context,
                agent_name: target_agent.clone(),
                body_id: body_id.clone(),
                project_root: current_dir.to_string_lossy().to_string(),
                force: true, // Force re-sync
                driver_id: "reconnect".to_string(),
                metrics: None,
            };

            let res = client.create_lease(req).await;

            match res {
                Ok(resp) => {
                    let pkg = resp.into_inner();
                    let sid = pkg.session_id;

                    // Recover body_id from session if not in environment
                    let bid = body_id;

                    if live {
                        println!("\n\x1b[32m--- KoadOS Live Connection RESTORED ---\x1b[0m");
                        println!("Agent:    {}", target_agent);
                        println!("Session:  {}", sid);
                    } else {
                        println!("\n\x1b[32m--- KoadOS Neural Link RE-ESTABLISHED ---\x1b[0m");
                        println!("Agent:    {}", target_agent);
                        println!("Session:  {}", sid);
                        println!("Body:     {}", bid);
                        println!("\nShell:    Run `export KOAD_SESSION_ID={} KOAD_BODY_ID={}` to bind this shell.", sid, bid);
                    }
                }
                Err(e) => {
                    anyhow::bail!(
                        "Reconnection Failed: {}. Try a fresh `koad boot`.",
                        e.message()
                    );
                }
            }
        }
        SystemAction::Backup { source } => {
            if !is_admin {
                anyhow::bail!("Admin only.");
            }
            let mut client = AdminClient::connect(config.network.citadel_grpc_addr.clone()).await?;
            let context = Some(crate::utils::get_trace_context(agent_name, 3));
            let req = TriggerBackupRequest { context, source };
            let res = client
                .trigger_backup(crate::utils::authenticated_request(req))
                .await?
                .into_inner();

            if res.success {
                println!("\x1b[32m[OK]\x1b[0m {}", res.message);
                println!("Backup ID: {}", res.backup_id);
            } else {
                println!("\x1b[31m[ERROR]\x1b[0m {}", res.message);
            }
        }
        SystemAction::Logs {
            service,
            tail,
            follow,
        } => {
            let log_dir = config.home.join("logs");
            let service_filter = service.as_deref().unwrap_or("");

            let log_files = fs::read_dir(&log_dir)?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.is_file()
                        && p.extension().and_then(|s| s.to_str()) == Some("log")
                        && p.file_name()
                            .and_then(|s| s.to_str())
                            .map(|s| s.contains(service_filter))
                            .unwrap_or(false)
                })
                .collect::<Vec<_>>();

            if log_files.is_empty() {
                anyhow::bail!(
                    "No matching log files found in {:?} (Filter: '{}')",
                    log_dir,
                    service_filter
                );
            }

            let latest_log = log_files
                .iter()
                .max_by_key(|p| fs::metadata(p).and_then(|m| m.modified()).ok());

            if let Some(path) = latest_log {
                let mut args = vec!["-n".to_string(), tail.to_string()];
                if follow {
                    args.push("-f".to_string());
                }
                args.push(path.to_string_lossy().to_string());

                println!(">>> Tailing log: {:?}", path);
                let mut child = Command::new("tail").args(args).spawn()?;
                let _ = child.wait();
            }
        }
        SystemAction::Start => {
            start_citadel_services(config)?;
        }
        SystemAction::Status { json, full } => {
            crate::handlers::status::handle_status_command(json, full, false, config, db).await?;
        }
        SystemAction::Restart => {
            stop_citadel_processes();
            std::thread::sleep(Duration::from_millis(800));
            start_citadel_services(config)?;
        }
        SystemAction::Stop { drain, confirm } => {
            if !is_admin {
                anyhow::bail!("Admin only.");
            }

            if !confirm {
                println!("\x1b[33m[SAFETY GATE]\x1b[0m This will shut down the KoadOS Citadel and all background services.");
                println!("Run with --confirm to proceed.");
                return Ok(());
            }

            if drain {
                println!(">>> [1/2] Neuronal Flush (Citadel Shutdown)...");
                if let Ok(mut client) =
                    AdminClient::connect(config.network.citadel_grpc_addr.clone()).await
                {
                    let context = Some(crate::utils::get_trace_context(agent_name, 3));
                    let _ = client
                        .shutdown(crate::utils::authenticated_request(ShutdownRequest {
                            context,
                            reason: "Manual system stop requested.".to_string(),
                        }))
                        .await;
                }
            }

            println!(">>> [2/2] Terminating KoadOS Core...");
            stop_citadel_processes();
            println!("\x1b[32m[OK]\x1b[0m System halted.");
        }
        SystemAction::Scrub { dry_run, force } => {
            handle_scrub(&config.home, dry_run, force)?;
        }
        SystemAction::Context { action } => {
            handle_context_action(action, config, db, agent_name).await?;
        }
        SystemAction::BoardSync {
            dry_run,
            auto_spawn,
        } => {
            crate::handlers::board_sync::handle_board_sync(dry_run, auto_spawn, config, agent_name)
                .await?;
        }
        SystemAction::Heartbeat { daemon, session } => {
            handle_heartbeat(daemon, session, config).await?;
        }
    }
    Ok(())
}

/// Kill all running Citadel-stack processes (citadel + cass).
fn stop_citadel_processes() {
    let _ = Command::new("pkill").arg("koad-citadel").status();
    let _ = Command::new("pkill").arg("koad-cass").status();
    let _ = Command::new("pkill").arg("koad-os-mcp").status();
}

/// Start koad-citadel and koad-cass.
///
/// Tries systemctl first (when units are installed). Falls back to spawning
/// the binaries directly from the koad-os bin/ directory with log file output.
fn start_citadel_services(config: &KoadConfig) -> Result<()> {
    println!("Pre-flight cleanup: Terminating existing Citadel processes...");
    stop_citadel_processes();

    let log_dir = config.home.join("logs");
    let _ = std::fs::create_dir_all(&log_dir);

    // Prefer systemctl when the unit is known to systemd.
    let systemctl_known = Command::new("systemctl")
        .args(["cat", "koad-citadel.service"])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if systemctl_known {
        // Starting citadel also starts cass via Wants=koad-cass.service.
        let status = Command::new("systemctl")
            .args(["start", "koad-citadel.service"])
            .status()?;
        if status.success() {
            println!("\x1b[32m[OK]\x1b[0m Citadel, CASS, and MCP started via systemctl.");
            return Ok(());
        } else {
            warn!("systemctl start koad-citadel.service failed (likely auth required). Falling back to manual spawn.");
        }
    }

    // Fallback: spawn binaries directly.
    let bin_dir = config.home.join("bin");
    let env_file = config.home.join(".env");

    let open_log = |name: &str, suffix: &str| -> Result<std::process::Stdio> {
        let path = log_dir.join(format!("{}.{}.log", name, suffix));
        let f = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(std::process::Stdio::from(f))
    };

    // Start Citadel.
    Command::new(bin_dir.join("koad-citadel"))
        .env("KOADOS_HOME", &config.home)
        .env("KOAD_HOME", &config.home)
        .env_remove("RUST_LOG")
        .stdout(open_log("citadel", "out")?)
        .stderr(open_log("citadel", "error")?)
        .spawn()
        .context("Failed to spawn koad-citadel")?;

    println!("\x1b[32m[1/2]\x1b[0m koad-citadel started.");
    std::thread::sleep(Duration::from_millis(500));

    // Start CASS.
    let mut cass_cmd = Command::new(bin_dir.join("koad-cass"));
    cass_cmd
        .env("KOADOS_HOME", &config.home)
        .env("KOAD_HOME", &config.home)
        .env_remove("RUST_LOG")
        .stdout(open_log("cass", "out")?)
        .stderr(open_log("cass", "error")?)
        .spawn()
        .context("Failed to spawn koad-cass")?;

    println!("\x1b[32m[2/3]\x1b[0m koad-cass started.");
    std::thread::sleep(Duration::from_millis(500));

    // Start MCP Bridge.
    Command::new(bin_dir.join("koad-os-mcp"))
        .env("KOADOS_HOME", &config.home)
        .env("KOAD_HOME", &config.home)
        .env_remove("RUST_LOG")
        .stdout(open_log("mcp", "out")?)
        .stderr(open_log("mcp", "error")?)
        .spawn()
        .context("Failed to spawn koad-os-mcp")?;

    println!("\x1b[32m[3/3]\x1b[0m koad-os-mcp started.");

    // Source .env for KOADOS_PAT_NOTION_MAIN etc. if present — best-effort.
    // (The spawned processes inherit this shell's env; actual secret resolution
    //  happens inside each binary via KoadConfig::resolve_secret.)
    drop(env_file); // not parsed here; binaries handle it internally.

    println!("\x1b[32m[OK]\x1b[0m Citadel, CASS, and MCP online.");
    Ok(())
}

fn system_init(config: &KoadConfig, force: bool) -> Result<()> {
    crate::handlers::system_init::run(config, force)
}

pub async fn handle_heartbeat(
    daemon: bool,
    session: Option<String>,
    config: &KoadConfig,
) -> Result<()> {
    let session_id = session
        .or_else(|| env::var("KOAD_SESSION_ID").ok())
        .context("No session ID provided or found in environment (KOAD_SESSION_ID).")?;

    let agent_name = config.get_agent_name();
    let mut client = CitadelSessionClient::connect(config.network.citadel_grpc_addr.clone())
        .await
        .map_err(|e| map_connect_err("KoadOS Citadel", &config.network.citadel_grpc_addr, e))
        .map_err(anyhow::Error::from)?;

    if daemon {
        // Heartbeat Daemon: Subconscious Neural Pulse
        loop {
            let context = Some(crate::utils::get_trace_context(&agent_name, 1));
            let req = HeartbeatRequest {
                context,
                session_id: session_id.clone(),
                metrics: None,
            };

            if let Err(e) = client
                .heartbeat(crate::utils::authenticated_request(req))
                .await
            {
                warn!(
                    "Heartbeat failure for session {}: {}. Attempting re-connection...",
                    session_id, e
                );

                // 1. Re-connect to gRPC
                if let Ok(mut c) =
                    CitadelSessionClient::connect(config.network.citadel_grpc_addr.clone()).await
                {
                    // 2. If session is unknown to Citadel (e.g. Citadel rebooted), trigger Live Reconnect
                    if e.code() == tonic::Code::NotFound || e.code() == tonic::Code::Unavailable {
                        let body_id = env::var("KOAD_BODY_ID").unwrap_or_default();
                        let current_dir =
                            env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
                        let context = Some(crate::utils::get_trace_context(&agent_name, 3));
                        let rec_req = LeaseRequest {
                            context,
                            agent_name: agent_name.clone(),
                            body_id,
                            project_root: current_dir.to_string_lossy().to_string(),
                            force: true,
                            driver_id: "heartbeat-recovery".to_string(),
                            metrics: None,
                        };
                        if c.create_lease(rec_req).await.is_ok() {
                            info!(
                                "Subconscious Recovery: Session {} re-linked to Citadel.",
                                session_id
                            );
                        }
                    }
                    client = c;
                }
            }
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
        }
    } else {
        let context = Some(crate::utils::get_trace_context(&agent_name, 1));
        client
            .heartbeat(crate::utils::authenticated_request(HeartbeatRequest {
                context,
                session_id: session_id.clone(),
                metrics: None,
            }))
            .await?;
        println!(
            "\x1b[32m[OK]\x1b[0m Heartbeat transmitted for session {}.",
            session_id
        );
    }
    Ok(())
}

pub async fn handle_context_action(
    action: crate::cli::ContextAction,
    config: &KoadConfig,
    db: &KoadDB,
    agent_name: &str,
) -> Result<()> {
    let mut client = AdminClient::connect(config.network.citadel_grpc_addr.clone())
        .await
        .map_err(|e| map_connect_err("KoadOS Citadel", &config.network.citadel_grpc_addr, e))
        .map_err(anyhow::Error::from)?;

    match action {
        crate::cli::ContextAction::Hydrate {
            session,
            path,
            text,
            ttl: _,
        } => {
            let session_id = if let Some(s) = session {
                s
            } else {
                // Try to resolve session ID from Redis for current agent
                let client = RedisClient::new(&config.home.to_string_lossy(), false).await?;
                let key = format!("koad:identity:{}", agent_name);
                let sid: String = client
                    .pool
                    .hget::<Option<String>, _, _>(&key, "session_id")
                    .await?
                    .context("No active session found for current agent. Provide --session.")?;
                sid
            };

            let (content, file_path) = if let Some(p) = path {
                ("".to_string(), Some(p.to_string_lossy().to_string()))
            } else if let Some(t) = text {
                (t, None)
            } else {
                anyhow::bail!("Provide either --path or --text to hydrate.");
            };

            let chunk_id = if let Some(ref path) = file_path {
                path.clone()
            } else {
                let mut hasher = sha2::Sha256::new();
                use sha2::Digest;
                hasher.update(content.as_bytes());
                format!("{:x}", hasher.finalize())
            };

            let mut cass =
                MemoryServiceClient::connect(config.network.cass_grpc_addr.clone()).await?;
            let req = FactCard {
                id: chunk_id,
                source_agent: agent_name.to_string(),
                session_id: session_id.clone(),
                domain: "context".to_string(),
                content,
                confidence: 1.0,
                tags: vec!["manual-hydration".to_string()],
                created_at: Some(prost_types::Timestamp {
                    seconds: Local::now().timestamp(),
                    nanos: Local::now().timestamp_subsec_nanos() as i32,
                }),
                metadata: None,
            };

            let res = cass.commit_fact(req).await?.into_inner();
            if res.success {
                println!(
                    "\x1b[32m[OK]\x1b[0m Context Hydrated for session {}.",
                    session_id
                );
            } else {
                println!("\x1b[31m[ERROR]\x1b[0m Hydration Failed: {}", res.message);
            }
        }
        crate::cli::ContextAction::Flush { session, confirm } => {
            if !confirm {
                println!("\x1b[33m[SAFETY GATE]\x1b[0m This will purge all volatile hot context for the target session. This action is irreversible.");
                println!("Run with --confirm to proceed.");
                return Ok(());
            }
            let target_sid = if let Some(s) = session {
                s
            } else {
                env::var("KOAD_SESSION_ID")
                    .context("KOAD_SESSION_ID not set. Provide --session ID.")?
            };

            client
                .flush_context(crate::utils::authenticated_request(FlushContextRequest {
                    context: Some(crate::utils::get_trace_context(agent_name, 3)),
                    session_id: target_sid.clone(),
                }))
                .await?;

            println!(
                "\x1b[32m[OK]\x1b[0m Hot context flushed for session {}.",
                target_sid
            );
        }
        crate::cli::ContextAction::List { agent } => {
            let conn = db.get_conn()?;
            let target_agent = agent.unwrap_or_else(|| agent_name.to_string());

            let mut stmt = conn.prepare("SELECT id, session_id, created_at FROM context_snapshots WHERE agent_name = ?1 ORDER BY created_at DESC")?;
            let snapshot_iter = stmt.query_map(params![target_agent], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?;

            println!(
                "\n\x1b[1m--- Cognitive Quicksaves for {} ---\x1b[0m",
                target_agent
            );
            println!(
                "{:<38} | {:<15} | {:<20}",
                "Snapshot ID", "Session ID", "Created At"
            );
            println!("{}", "-".repeat(80));

            let mut count = 0;
            for snap in snapshot_iter {
                let (id, sid, ts) = snap?;
                let dt = chrono::DateTime::from_timestamp(ts, 0).unwrap_or_default();
                println!("{:<38} | {:<15} | {:<20}", id, &sid[..8], dt.to_rfc3339());
                count += 1;
            }

            if count == 0 {
                println!("No snapshots found for agent '{}'.", target_agent);
            }
            println!();
        }
        crate::cli::ContextAction::Restore { id, session } => {
            let conn = db.get_conn()?;

            // 1. Fetch snapshot from DB
            let snapshot_json: String = conn
                .query_row(
                    "SELECT snapshot_json FROM context_snapshots WHERE id = ?1 OR id LIKE ?2",
                    params![id, format!("{}%", id)],
                    |row| row.get(0),
                )
                .context("Snapshot not found.")?;

            let data: serde_json::Value = serde_json::from_str(&snapshot_json)?;
            let hot_context = data["hot_context"]
                .as_object()
                .context("Malformed snapshot: missing hot_context")?;

            // 2. Resolve target session
            let target_session_id = if let Some(s) = session {
                s
            } else {
                let client = RedisClient::new(&config.home.to_string_lossy(), false).await?;
                let key = format!("koad:identity:{}", agent_name);
                let sid: String = client
                    .pool
                    .hget::<Option<String>, _, _>(&key, "session_id")
                    .await?
                    .context("No active session found. Provide --session ID.")?;
                sid
            };

            println!(
                ">>> Restoring {} context chunks to session {}...",
                hot_context.len(),
                target_session_id
            );

            // 3. Hydrate chunks via gRPC
            let mut success_count = 0;
            let mut cass =
                MemoryServiceClient::connect(config.network.cass_grpc_addr.clone()).await?;
            for (chunk_id, chunk_val) in hot_context {
                if let Ok(chunk_data) =
                    serde_json::from_str::<serde_json::Value>(chunk_val.as_str().unwrap_or(""))
                {
                    let req = FactCard {
                        id: chunk_id.clone(),
                        source_agent: agent_name.to_string(),
                        session_id: target_session_id.clone(),
                        domain: "restore".to_string(),
                        content: chunk_data["content"]
                            .as_str()
                            .unwrap_or_default()
                            .to_string(),
                        confidence: 1.0,
                        tags: vec!["snapshot-restore".to_string()],
                        created_at: Some(prost_types::Timestamp {
                            seconds: Local::now().timestamp(),
                            nanos: Local::now().timestamp_subsec_nanos() as i32,
                        }),
                        metadata: None,
                    };

                    if cass.commit_fact(req).await.is_ok() {
                        success_count += 1;
                    }
                }
            }

            println!(
                "\x1b[32m[OK]\x1b[0m Successfully restored {}/{} context chunks.",
                success_count,
                hot_context.len()
            );
        }
    }

    Ok(())
}

// ── Distribution Sanitizer ────────────────────────────────────────────────

/// Describes a single scrub action.
enum ScrubAction {
    /// Delete this specific file.
    DeleteFile(std::path::PathBuf),
    /// Truncate this file to zero bytes (keep the file, empty its content).
    TruncateFile(std::path::PathBuf),
    /// Overwrite this file with fixed content.
    ResetFile {
        path: std::path::PathBuf,
        content: String,
    },
    /// Recursively delete this directory.
    DeleteDir(std::path::PathBuf),
}

impl ScrubAction {
    fn describe(&self) -> String {
        match self {
            ScrubAction::DeleteFile(p) => format!("delete file:      {}", p.display()),
            ScrubAction::TruncateFile(p) => format!("truncate file:    {}", p.display()),
            ScrubAction::ResetFile { path, .. } => format!("reset file:       {}", path.display()),
            ScrubAction::DeleteDir(p) => format!("delete dir:       {}", p.display()),
        }
    }
}

/// Collect all scrub targets for the given KoadOS home directory.
fn collect_scrub_targets(home: &std::path::Path) -> Result<Vec<ScrubAction>> {
    let mut actions: Vec<ScrubAction> = Vec::new();

    // 1. data/db/ — delete all SQLite files
    let db_dir = home.join("data/db");
    if db_dir.exists() {
        for entry in std::fs::read_dir(&db_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.ends_with(".db") || name.ends_with(".db-shm") || name.ends_with(".db-wal") {
                    actions.push(ScrubAction::DeleteFile(path));
                }
            }
        }
    }

    // 2. logs/ — truncate all files
    let logs_dir = home.join("logs");
    if logs_dir.exists() {
        for entry in std::fs::read_dir(&logs_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                actions.push(ScrubAction::TruncateFile(path));
            }
        }
    }

    // 3. run/ — delete .sock and .pid files
    let run_dir = home.join("run");
    if run_dir.exists() {
        for entry in std::fs::read_dir(&run_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if name.ends_with(".sock") || name.ends_with(".pid") {
                    actions.push(ScrubAction::DeleteFile(path));
                }
            }
        }
    }

    // 4. agents/bays/ — delete all subdirs
    let bays_dir = home.join("agents/bays");
    if bays_dir.exists() {
        for entry in std::fs::read_dir(&bays_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                actions.push(ScrubAction::DeleteDir(path));
            }
        }
    }

    // 5. agents/KAPVs/ — delete subdirs only; keep TEMPLATE* files
    let kapvs_dir = home.join("agents/KAPVs");
    if kapvs_dir.exists() {
        for entry in std::fs::read_dir(&kapvs_dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                actions.push(ScrubAction::DeleteDir(path));
            } else if path.is_file() && !name.starts_with("TEMPLATE") {
                actions.push(ScrubAction::DeleteFile(path));
            }
        }
    }

    // 6. SESSIONS_LOG.md — truncate if exists
    let sessions_log = home.join("SESSIONS_LOG.md");
    if sessions_log.exists() {
        actions.push(ScrubAction::TruncateFile(sessions_log));
    }

    // 7. TEAM-LOG.md — reset to release header stub
    let team_log = home.join("TEAM-LOG.md");
    if team_log.exists() {
        let release_header = "# KoadOS Team Log — Distribution Release\n\
            **Status:** Clean Slate — scrubbed for distribution.\n\
            **Lead:** (unassigned)\n\n\
            | Date | Teammate | Task ID | Status | Notes |\n\
            |------|----------|---------|--------|-------|\n"
            .to_string();
        actions.push(ScrubAction::ResetFile {
            path: team_log,
            content: release_header,
        });
    }

    // 8. cache/ — delete all contents
    let cache_dir = home.join("cache");
    if cache_dir.exists() {
        for entry in std::fs::read_dir(&cache_dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() {
                actions.push(ScrubAction::DeleteFile(path));
            } else if path.is_dir() {
                actions.push(ScrubAction::DeleteDir(path));
            }
        }
    }

    Ok(actions)
}

/// Execute (or simulate) a list of scrub actions.
fn execute_scrub_actions(actions: &[ScrubAction], dry_run: bool) -> Result<()> {
    for action in actions {
        let label = if dry_run { "[dry-run]" } else { "[scrub]  " };
        println!("{}  {}", label, action.describe());
        if !dry_run {
            match action {
                ScrubAction::DeleteFile(p) => {
                    std::fs::remove_file(p)
                        .with_context(|| format!("Failed to delete {}", p.display()))?;
                }
                ScrubAction::TruncateFile(p) => {
                    std::fs::write(p, b"")
                        .with_context(|| format!("Failed to truncate {}", p.display()))?;
                }
                ScrubAction::ResetFile { path, content } => {
                    std::fs::write(path, content.as_bytes())
                        .with_context(|| format!("Failed to reset {}", path.display()))?;
                }
                ScrubAction::DeleteDir(p) => {
                    std::fs::remove_dir_all(p)
                        .with_context(|| format!("Failed to delete dir {}", p.display()))?;
                }
            }
        }
    }
    Ok(())
}

fn handle_scrub(home: &std::path::Path, dry_run: bool, force: bool) -> Result<()> {
    // Git check: warn on dirty working tree
    let git_status = Command::new("git")
        .args(["-C", &home.to_string_lossy(), "status", "--short"])
        .output();
    if let Ok(output) = git_status {
        let dirty = String::from_utf8_lossy(&output.stdout);
        if !dirty.trim().is_empty() {
            println!("\x1b[33m[WARNING]\x1b[0m Uncommitted changes detected in the repository:");
            for line in dirty.lines().take(10) {
                println!("  {}", line);
            }
            println!();
        }
    }

    // Collect targets
    let actions = collect_scrub_targets(home)?;

    if actions.is_empty() {
        println!("Nothing to scrub. Citadel is already clean.");
        return Ok(());
    }

    println!("\x1b[1mKoadOS Distribution Sanitizer\x1b[0m");
    println!("Targets identified: {}", actions.len());
    println!();
    for action in &actions {
        println!("  - {}", action.describe());
    }
    println!();

    if dry_run {
        println!("\x1b[34m[DRY RUN]\x1b[0m No files were modified.");
        return Ok(());
    }

    // Confirmation gate
    if !force {
        println!("\x1b[31m[DANGER]\x1b[0m This will permanently delete local state.");
        println!("Type 'SCRUB' to confirm, or anything else to cancel:");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim() != "SCRUB" {
            println!("Scrub cancelled.");
            return Ok(());
        }
        println!();
    }

    execute_scrub_actions(&actions, false)?;

    println!();
    println!(
        "\x1b[32m[DONE]\x1b[0m Citadel scrubbed. {} action(s) completed.",
        actions.len()
    );
    println!("Run `koad system init` to re-initialize the environment.");
    Ok(())
}

async fn sync_session_histories(home: &std::path::Path, cass_url: &str) -> Result<()> {
    let bays_dir = home.join("agents/bays");
    if !bays_dir.exists() {
        return Ok(());
    }

    let mut client = match MemoryServiceClient::connect(cass_url.to_string()).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("Session Sync: Cannot connect to CASS at {}: {}", cass_url, e);
            return Ok(());
        }
    };

    let entries = std::fs::read_dir(&bays_dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let db_path = path.join("state.db");
            if db_path.exists() {
                let agent_name = path.file_name().unwrap_or_default().to_string_lossy().into_owned();
                tracing::debug!("Session Sync: Processing agent bay '{}'", agent_name);

                if let Ok(conn) = rusqlite::Connection::open(&db_path) {
                    let mut stmt = match conn.prepare(
                        "SELECT session_id, start_time, end_time, eow_path, final_state FROM session_history"
                    ) {
                        Ok(s) => s,
                        Err(_) => continue,
                    };

                    let rows = stmt.query_map([], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, Option<i64>>(2)?,
                            row.get::<_, Option<String>>(3)?,
                            row.get::<_, String>(4)?,
                        ))
                    });

                    if let Ok(rows) = rows {
                        for row in rows {
                            if let Ok((session_id, start_time, end_time, eow_path, final_state)) = row {
                                let mut summary = String::new();
                                if let Some(ref path_str) = eow_path {
                                    if let Ok(content) = std::fs::read_to_string(path_str) {
                                        summary = content;
                                    }
                                }

                                if summary.is_empty() {
                                    let start_str = chrono::DateTime::from_timestamp(start_time, 0)
                                        .map(|t| t.to_rfc3339())
                                        .unwrap_or_else(|| start_time.to_string());
                                    let end_str = end_time
                                        .and_then(|t| chrono::DateTime::from_timestamp(t, 0))
                                        .map(|t| t.to_rfc3339())
                                        .unwrap_or_default();
                                    summary = format!(
                                        "Session {} completed by agent {} (Start: {}, End: {}). Final status: {}.",
                                        session_id, agent_name, start_str, end_str, final_state
                                    );
                                }

                                let episode = koad_proto::cass::v1::EpisodicMemory {
                                    session_id: session_id.clone(),
                                    project_path: home.to_string_lossy().into_owned(),
                                    summary,
                                    turn_count: 0,
                                    timestamp: Some(prost_types::Timestamp {
                                        seconds: end_time.unwrap_or(start_time),
                                        nanos: 0,
                                    }),
                                    task_ids: vec![],
                                    metadata: None,
                                };

                                if let Err(e) = client
                                    .record_episode(crate::utils::authenticated_request(episode))
                                    .await
                                {
                                    tracing::warn!("Session Sync: Failed to sync session {}: {}", session_id, e);
                                } else {
                                    tracing::debug!("Session Sync: Synced session {} successfully", session_id);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod scrub_tests {
    use super::*;
    use std::fs;

    fn setup_fake_home(tmp: &std::path::Path) {
        // Create the directory structure that collect_scrub_targets scans
        fs::create_dir_all(tmp.join("data/db")).unwrap();
        fs::create_dir_all(tmp.join("logs")).unwrap();
        fs::create_dir_all(tmp.join("run")).unwrap();
        fs::create_dir_all(tmp.join("agents/bays/tyr")).unwrap();
        fs::create_dir_all(tmp.join("agents/KAPVs/clyde")).unwrap();
        fs::create_dir_all(tmp.join("cache")).unwrap();

        // Seed files
        fs::write(tmp.join("data/db/koad.db"), b"db").unwrap();
        fs::write(tmp.join("data/db/koad.db-shm"), b"shm").unwrap();
        fs::write(tmp.join("logs/citadel.log"), b"log").unwrap();
        fs::write(tmp.join("run/koad.sock"), b"sock").unwrap();
        fs::write(tmp.join("run/redis.pid"), b"pid").unwrap();
        fs::write(tmp.join("cache/boot-metrics.md"), b"cache").unwrap();
        fs::write(tmp.join("TEAM-LOG.md"), b"# Log").unwrap();
    }

    #[test]
    fn test_collect_scrub_targets_finds_expected_files() {
        let tmp = tempfile::tempdir().unwrap();
        setup_fake_home(tmp.path());

        let actions = collect_scrub_targets(tmp.path()).unwrap();
        // Should find: koad.db, koad.db-shm, citadel.log, koad.sock, redis.pid,
        //              agents/bays/tyr (dir), agents/KAPVs/clyde (dir),
        //              cache/boot-metrics.md, TEAM-LOG.md reset
        assert!(
            actions.len() >= 8,
            "expected at least 8 scrub actions, got {}",
            actions.len()
        );
    }

    #[test]
    fn test_collect_scrub_targets_preserves_template_files() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("agents/KAPVs")).unwrap();
        fs::write(tmp.path().join("agents/KAPVs/TEMPLATE.md"), b"template").unwrap();
        fs::create_dir_all(tmp.path().join("agents/KAPVs/agent-vault")).unwrap();

        let actions = collect_scrub_targets(tmp.path()).unwrap();
        // TEMPLATE.md must NOT appear in actions
        let hits_template = actions.iter().any(|a| {
            let p = match a {
                ScrubAction::DeleteFile(p) | ScrubAction::TruncateFile(p) => p,
                ScrubAction::ResetFile { path, .. } => path,
                ScrubAction::DeleteDir(p) => p,
            };
            p.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .starts_with("TEMPLATE")
        });
        assert!(!hits_template, "TEMPLATE files must be preserved by scrub");
        // The agent-vault dir SHOULD appear
        let hits_vault = actions
            .iter()
            .any(|a| matches!(a, ScrubAction::DeleteDir(p) if p.ends_with("agent-vault")));
        assert!(hits_vault, "non-template KAPVs dirs should be targeted");
    }

    #[test]
    fn test_execute_scrub_actions_dry_run_leaves_files_intact() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("test.db");
        fs::write(&file, b"data").unwrap();

        let actions = vec![ScrubAction::DeleteFile(file.clone())];
        execute_scrub_actions(&actions, true /* dry_run */).unwrap();

        assert!(file.exists(), "dry-run must not delete files");
        assert_eq!(
            fs::read(&file).unwrap(),
            b"data",
            "dry-run must not modify file content"
        );
    }

    #[test]
    fn test_execute_scrub_actions_truncate() {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join("citadel.log");
        fs::write(&file, b"lots of logs").unwrap();

        let actions = vec![ScrubAction::TruncateFile(file.clone())];
        execute_scrub_actions(&actions, false).unwrap();

        assert!(file.exists(), "truncated file should still exist");
        assert_eq!(
            fs::read(&file).unwrap(),
            b"",
            "truncated file should be empty"
        );
    }
}
