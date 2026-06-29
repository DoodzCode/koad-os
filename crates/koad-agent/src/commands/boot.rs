use anyhow::{Context, Result};
use koad_core::config::KoadConfig;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::process::Command;
use std::time::Duration;

use koad_proto::cass::v1::hydration_service_client::HydrationServiceClient;
use koad_proto::cass::v1::HydrationRequest;
use koad_proto::citadel::v5::citadel_session_client::CitadelSessionClient;
use koad_proto::citadel::v5::{LeaseRequest, TraceContext, WorkspaceLevel};
use tonic::transport::Endpoint;

use crate::commands::verify::verify_kapv;

/// Timeout for boot-path gRPC connections to local Citadel/CASS services.
const BOOT_SERVICE_TIMEOUT: Duration = Duration::from_secs(3);

pub async fn handle_boot(
    config: &KoadConfig,
    agent: Option<String>,
    name: Option<String>,
    shell: bool,
) -> Result<()> {
    let boot_start = std::time::Instant::now();
    let agent_name = agent.or(name).context("No agent name provided. Use 'koad-agent boot <name>' or 'koad-agent boot --agent <name>'.")?;

    let agent_key = agent_name.to_lowercase();
    let identity_config = config.identities.get(&agent_key);

    // --- [Pre-flight: Body Check] ---
    if let Some(id) = identity_config {
        if let Some(required_runtime) = &id.runtime {
            let active_runtime = std::env::var("KOAD_RUNTIME").unwrap_or_default();
            if active_runtime.to_lowercase() != required_runtime.to_lowercase() {
                eprintln!(
                    "\x1b[31m[BOOT DENIED]\x1b[0m No agent body detected for '{}'.",
                    agent_name
                );
                eprintln!(
                "  Required runtime: \x1b[33m{}\x1b[0m — set KOAD_RUNTIME={} to authorize.",
                required_runtime, required_runtime
            );
                std::process::exit(1);
            }
        }
    }

    let vault_uri = config
        .resolve_vault_uri(&agent_name)
        .context("Could not resolve vault URI for current agent.")?;
    let vault_path = match config.resolve_vault_path(&vault_uri) {
        Ok(p) => p,
        Err(e) => {
            if shell {
                println!("echo \"\x1b[31m[ERROR]\x1b[0m {}\";", e);
                return Ok(());
            } else {
                return Err(e);
            }
        }
    };

    if let Err(e) = verify_kapv(&vault_path).await {
        if shell {
            println!(
                "echo \"\x1b[31m[ERROR]\x1b[0m Vault verification failed for '{}': {}\";",
                agent_name, e
            );
            return Ok(());
        } else {
            return Err(e);
        }
    }

    if shell {
        let mut cass_packet_size = 0;
        let mut boot_status = "OK";
        let mut cass_packet = String::new();
        let git_status;
        let now = chrono::Utc::now();
        let timestamp = now.to_rfc3339();

        let mut hasher = DefaultHasher::new();
        timestamp.hash(&mut hasher);
        let cache_hash = hasher.finish();

        println!("export KOADOS_HOME=\"{}\";", config.home.display());
        println!("export KOAD_AGENT_NAME=\"{}\";", agent_name);
        println!("export KOAD_VAULT_URI=\"{}\";", vault_uri);
        println!("export KOAD_VAULT_PATH=\"{}\";", vault_path.display());
        println!("export KOAD_BANK_PATH=\"{}/bank\";", vault_path.display());
        println!(
            "export HISTFILE=\"{}/sessions/bash_history\";",
            vault_path.display()
        );
        println!("export TMPDIR=\"{}/bank/tmp\";", vault_path.display());
        println!("export KOAD_PROMPT_CACHE_HASH=\"{}\";", cache_hash);
        println!("export KOAD_BOOT_MODE=\"dark\";");
        println!("export CASS_GRPC_ADDR=\"{}\";", config.network.cass_grpc_addr);

        let agent_key = agent_name.to_lowercase();

        // WSL GPU/CUDA path fix
        if Path::new("/usr/lib/wsl/lib").exists() {
            println!(
                "export LD_LIBRARY_PATH=\"/usr/lib/wsl/lib${{LD_LIBRARY_PATH:+:${{LD_LIBRARY_PATH}}}}\";"
            );
        }

        // Fast Display: Show the last known state immediately
        let cache_dir = config.home.join("cache");
        let brief_cache = cache_dir.join(format!("session-brief-{}.md", agent_key));
        if brief_cache.exists() {
            println!(
                "echo -e \"\\x1b[1;30m[QUICK-RESTORE] Loading last cached brief...\\x1b[0m\";"
            );
            println!("cat \"{}\";", brief_cache.display());
            println!(
                "echo -e \"\\x1b[1;30m-------------------------------------------\\x1b[0m\";"
            );
        }
        let home = dirs::home_dir().unwrap_or_default();

        if let Some(identity_config) = identity_config {
            println!("export KOAD_AGENT_ROLE=\"{}\";", identity_config.role);
            println!("export KOAD_AGENT_RANK=\"{}\";", identity_config.rank);
            if let Some(rt) = &identity_config.runtime {
                println!("export KOAD_RUNTIME=\"{}\";", rt);
            }

            if let Some(prefs) = &identity_config.preferences {
                for key in &prefs.access_keys {
                    let resolved = config.resolve_secret(key, None);
                    if !resolved.is_empty() {
                        println!("export {}=\"{}\";", key, resolved);
                    }
                }
                
                if prefs
                    .access_keys
                    .iter()
                    .any(|k| k == "GITHUB_PAT" || k == "KOADOS_PAT_GITHUB_ADMIN")
                {
                    let github_owner = config.get_github_owner(None);
                    if !github_owner.is_empty() {
                        println!("export GITHUB_OWNER=\"{}\";", github_owner);
                    }
                    let github_repo = config.get_github_repo(None);
                    if !github_repo.is_empty() {
                        println!("export GITHUB_REPO=\"{}\";", github_repo);
                    }
                    println!("export GITHUB_PROJECT_NUMBER=2;");
                }
            }

            // --- [Parallel Phase 1: Handshakes & Data] ---
            let project_root = std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let citadel_addr = config.network.citadel_grpc_addr.clone();
            let cass_addr = config.network.cass_grpc_addr.clone();

            let agent_name_lease = agent_name.clone();
            let project_root_lease = project_root.clone();
            let agent_name_hydra = agent_name.clone();
            let project_root_hydra = project_root.clone();

            let lease_task = tokio::spawn(async move {
                match Endpoint::from_shared(citadel_addr.clone())
                    .unwrap()
                    .connect_timeout(BOOT_SERVICE_TIMEOUT)
                    .timeout(BOOT_SERVICE_TIMEOUT)
                    .connect()
                    .await
                {
                    Ok(channel) => {
                        let mut client = CitadelSessionClient::new(channel);
                        let mut request = tonic::Request::new(LeaseRequest {
                            context: Some(TraceContext {
                                trace_id: format!("BOOT-{}", cache_hash),
                                origin: "Bridge".to_string(),
                                actor: agent_name_lease.clone(),
                                timestamp: Some(prost_types::Timestamp {
                                    seconds: now.timestamp(),
                                    nanos: 0,
                                }),
                                level: WorkspaceLevel::LevelUnspecified as i32,
                            }),
                            agent_name: agent_name_lease.clone(),
                            project_root: project_root_lease,
                            force: true,
                            body_id: cache_hash.to_string(),
                            driver_id: "cli".to_string(),
                            metrics: None,
                        });

                        // Add mandatory Zero-Trust headers
                        request
                            .metadata_mut()
                            .insert("x-actor", agent_name_lease.parse().unwrap());
                        request
                            .metadata_mut()
                            .insert("x-session-id", "BOOT".parse().unwrap());
                        request
                            .metadata_mut()
                            .insert("x-session-token", "NONE".parse().unwrap());

                        match client.create_lease(request).await {
                            Ok(resp) => Some(resp),
                            Err(e) => {
                                eprintln!("{}", koad_core::utils::errors::map_status_err("KoadOS Citadel", e));
                                None
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!(
                            "{}",
                            koad_core::utils::errors::map_connect_err("KoadOS Citadel", &citadel_addr, e)
                        );
                        None
                    }
                }
            });

            let hydration_task = tokio::spawn(async move {
                let cass_addr_display = cass_addr.clone();
                match Endpoint::from_shared(cass_addr)
                    .unwrap()
                    .connect_timeout(BOOT_SERVICE_TIMEOUT)
                    .timeout(BOOT_SERVICE_TIMEOUT)
                    .connect()
                    .await
                {
                    Ok(channel) => {
                        let mut cass_client = HydrationServiceClient::new(channel);
                        let hydration_req = tonic::Request::new(HydrationRequest {
                            agent_name: agent_name_hydra,
                            project_root: project_root_hydra,
                            level: WorkspaceLevel::LevelUnspecified as i32,
                            token_budget: 4000,
                            task_id: String::new(),
                        });
                        cass_client.hydrate(hydration_req).await.ok()
                    }
                    Err(e) => {
                        eprintln!(
                            "{}",
                            koad_core::utils::errors::map_connect_err("KoadOS CASS", &cass_addr_display, e)
                        );
                        None
                    }
                }
            });

            let git_task = tokio::spawn(async move {
                Command::new("git")
                    .arg("status")
                    .arg("-s")
                    .output()
                    .await
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                    .unwrap_or_default()
            });

            let pulse_task = {
                let cass_addr = config.network.cass_grpc_addr.clone();
                let agent_name = agent_name.clone();
                let hasher_hash = cache_hash;
                tokio::spawn(async move {
                    match Endpoint::from_shared(cass_addr)
                        .unwrap()
                        .connect_timeout(BOOT_SERVICE_TIMEOUT)
                        .timeout(BOOT_SERVICE_TIMEOUT)
                        .connect()
                        .await
                    {
                        Ok(channel) => {
                            let mut client = koad_proto::cass::v1::pulse_service_client::PulseServiceClient::new(channel);
                            let req = koad_proto::cass::v1::GetPulsesRequest {
                                context: Some(koad_proto::citadel::v5::TraceContext {
                                    trace_id: format!("BOOT-PULSE-{}", hasher_hash),
                                    origin: "Bridge".to_string(),
                                    actor: agent_name,
                                    timestamp: None,
                                    level: koad_proto::citadel::v5::WorkspaceLevel::LevelUnspecified as i32,
                                }),
                                role: "global".to_string(),
                            };
                            client.get_pulses(req).await.ok().map(|r| r.into_inner().pulses)
                        }
                        Err(_) => None,
                    }
                })
            };

            let (lease_res, hydration_res, git_res, pulse_res) =
                tokio::join!(lease_task, hydration_task, git_task, pulse_task);

            if let Ok(Some(lease_response)) = lease_res {
                let res = lease_response.into_inner();
                println!("export KOAD_SESSION_ID=\"{}\";", res.session_id);
                println!("export KOAD_SESSION_TOKEN=\"{}\";", res.token);
            }

            if let Ok(Some(h_res)) = hydration_res {
                cass_packet = h_res.into_inner().markdown_packet;
                cass_packet_size = cass_packet.len();
            } else {
                boot_status = "FAIL (CASS/Hydration)";
            }

            let active_pulses = pulse_res.unwrap_or_default().unwrap_or_default();

            git_status = git_res.unwrap_or_default();

            // Telemetry (Phase 0)
            println!(
                "{}/scripts/koad-telemetry.sh boot {} {};",
                config.home.display(),
                agent_name,
                cache_hash
            );
            // Only set EXIT trap if not running in an agent harness (which intercepts/uses EXIT traps to preserve env)
            if std::env::var("ANTIGRAVITY_AGENT").is_err() && std::env::var("CLAUDE_CODE_ENTRYPOINT").is_err() {
                println!(
                    "trap \"{}/scripts/koad-telemetry.sh shutdown {} {}\" EXIT;",
                    config.home.display(),
                    agent_name,
                    cache_hash
                );
            }


            // --- AI Anchor Generation ---
            let mut anchor_content = format!(
            "# KoadOS Agent Identity Anchor\nGenerated At: {}\n\n## Identity\nName: {}\nRole: {}\nRank: {}\n\n## Bio\n{}\n\n## MANDATORY: Session Hydration\nIf you have not done so, or if you need to refresh your context, run:\n`source {}/bin/koad-functions.sh && agent-boot {}`\n",
            timestamp, identity_config.name, identity_config.role, identity_config.rank, identity_config.bio, config.home.display(), agent_key
        );

            // --- [AIS: Live Awareness Section] ---
            if !active_pulses.is_empty() {
                anchor_content.push_str("\n## 🛜 Live Awareness (Global Pulses)\n");
                for p in active_pulses {
                    anchor_content.push_str(&format!("- **{}**: {} \x1b[2m— {}\x1b[0m\n", p.author, p.message, p.role));
                }
            }

            anchor_content.push_str("\n## 📂 Filesystem Protocol: Scoped MCP\nAll filesystem operations MUST be performed via the `koadFsMcp` toolset (read_text_file, write_file, list_directory, etc.). Raw shell commands for file manipulation are strictly prohibited to ensure Sanctuary compliance.\n\n## 🧭 Navigation Protocol: Game Map HUD\nUse `koad map` for instant situational awareness. \n- `koad map look` → Describe surroundings & POIs.\n- `koad map exits` → Show available paths.\n- `koad map goto <alias>` → Fast-travel to pinned locations.\n- `koad map nearby` → Scan for related configs/tasks.\n\n## ⚡ Efficiency Policy: The 'No-Read' Rule\nTo minimize token burn, you are STRICTLY FORBIDDEN from reading entire source files unless they are under 50 lines. \n1. **Use your Context Packet:** Structural maps of relevant crates are provided in the CASS section below. Use them first.\n2. **Discovery:** Use `grep_search` to locate specific logic or patterns.\n3. **Targeted Reading:** Use `read_file` ONLY with `start_line` and `end_line` parameters for surgical extraction.\n");

            if !cass_packet.is_empty() {
                anchor_content.push_str("\n## 🧠 Temporal Context Packet (CASS)\n");
                anchor_content.push_str(&cass_packet);
            }

            // Resolve bootstrap path for the target agent
            let bootstrap_path = identity_config.bootstrap.as_ref().map(|b| {
                let path_str = if b.starts_with('~') {
                    b.replacen('~', &home.to_string_lossy(), 1)
                } else {
                    b.clone()
                };
                PathBuf::from(path_str)
            }).unwrap_or_else(|| {
                vault_path.join("GEMINI.md") // Default fallback inside the vault
            });

            // Ensure parent directory of bootstrap path exists
            if let Some(parent) = bootstrap_path.parent() {
                let _ = fs::create_dir_all(parent).await;
            }

            // Always write the full anchor to the agent's specific bootstrap path
            let _ = fs::write(bootstrap_path.clone(), &anchor_content).await;

            let is_gemini = identity_config.runtime.as_deref() == Some("gemini")
                || std::env::var("KOAD_RUNTIME").as_deref() == Ok("gemini");

            let is_claude = identity_config.runtime.as_deref() == Some("claude")
                || std::env::var("KOAD_RUNTIME").as_deref() == Ok("claude");

            if is_gemini {
                let router_content = format!(
                    "# KoadOS Agent Identity Router\n\
                     Generated At: {}\n\n\
                     ## Active Session Routing\n\
                     - **Active Agent:** {}\n\
                     - **Role:** {}\n\
                     - **Rank:** {}\n\
                     - **Agent Profile Vault (KAPV):** [vault](file://{})\n\
                     - **Bootstrap Anchor:** [bootstrap](file://{})\n\n\
                     ## Instructions\n\
                     You are routing to the active agent profile vault (KAPV). Please IMMEDIATELY load your full identity, system protocols, and temporal context from the bootstrap anchor file:\n\
                     [bootstrap](file://{})\n\n\
                     Follow all instructions, protocols, and context files located within your KAPV.\n\n\
                     ## MANDATORY: Session Hydration\n\
                     If you have not done so, or if you need to refresh your context, run:\n\
                     `source {}/bin/koad-functions.sh && agent-boot`\n",
                    timestamp,
                    identity_config.name,
                    identity_config.role,
                    identity_config.rank,
                    vault_path.display(),
                    bootstrap_path.display(),
                    bootstrap_path.display(),
                    config.home.display()
                );
                let _ = safe_write_anchor(home.join(".gemini/GEMINI.md"), &router_content, &identity_config.name).await;
                let _ = fs::write(PathBuf::from("GEMINI.md"), &router_content).await;
            } else if is_claude {
                let _ = safe_write_anchor(home.join(".claude/CLAUDE.md"), &anchor_content, &identity_config.name).await;
                let _ = fs::write(PathBuf::from("CLAUDE.md"), &anchor_content).await;
            } else {
                let _ = safe_write_anchor(home.join(".codex/AGENTS.md"), &anchor_content, &identity_config.name).await;
                let _ = fs::write(PathBuf::from("AGENTS.md"), &anchor_content).await;
            }
        } else {
            // If NO identity config, we still need a git_status for the brief below
            let git_task = tokio::spawn(async move {
                Command::new("git")
                    .arg("status")
                    .arg("-s")
                    .output()
                    .await
                    .ok()
                    .map(|o| String::from_utf8_lossy(&o.stdout).into_owned())
                    .unwrap_or_default()
            });
            git_status = git_task.await.unwrap_or_default();
        }

        // PATH Hydration
        let home = dirs::home_dir().unwrap_or_default();
        let cargo_bin = home.join(".cargo/bin");
        let koad_bin = config.home.join("bin");
        println!(
            "export PATH=\"{}:{}:$PATH\";",
            koad_bin.display(),
            cargo_bin.display()
        );

        // Session Brief
        let cache_dir = config.home.join("cache");
        let _ = fs::create_dir_all(&cache_dir).await;

        let mut brief_content = format!(
            "# Session Brief: {}\nGenerated At: {}\n\n## Git Status\n```\n{}\n```\n",
            agent_name,
            timestamp,
            git_status.trim()
        );

        // --- [ABC: Automated Boot Cognition] ---
        // We now use a centralized SITREP.md at the workspace root instead of isolated
        // working memory files. This ensures global consistency across the crew.
        if let Ok(sitrep) = std::fs::read_to_string("SITREP.md") {
            brief_content.push_str("\n## Tactical Brief (Citadel SITREP)\n");
            brief_content.push_str(&sitrep);
        }

        let _ = fs::write(
            cache_dir.join(format!("session-brief-{}.md", agent_key)),
            &brief_content,
        )
        .await;

        let boot_duration = boot_start.elapsed();
        let metrics_content = format!(
            "- **Hydration Time:** {:.2}ms\n- **CASS Packet Size:** {} bytes\n- **Status:** {}\n",
            boot_duration.as_secs_f64() * 1000.0,
            cass_packet_size,
            boot_status
        );
        let _ = fs::write(
            cache_dir.join(format!("boot-metrics-{}.md", agent_key)),
            &metrics_content,
        )
        .await;

        println!("function koad-refresh() {{ echo \"[REFRESH] Regenerating session brief...\"; eval $(koad-agent boot $KOAD_AGENT_NAME); }};");
        println!(
            "echo \"\x1b[1;34m--- KoadOS Session: {} ---\x1b[0m\";",
            agent_name
        );
        println!(
            "echo \"\x1b[32m[BOOT]\x1b[0m Neural link hydrated for agent '{}'.\";",
            agent_name
        );
    }
    Ok(())
}

/// Safely writes an identity anchor file, ensuring it doesn't overwrite
/// an active anchor for a DIFFERENT agent unless it is stale (>10m).
async fn safe_write_anchor(path: PathBuf, content: &str, agent_name: &str) -> Result<()> {
    if path.exists() {
        if let Ok(existing) = fs::read_to_string(&path).await {
            // Check if the current agent owns this file
            if existing.contains(&format!("Name: {}", agent_name)) {
                // Same agent, update allowed
                fs::write(&path, content).await?;
                return Ok(());
            }

            // Check age for staleness
            if let Ok(meta) = fs::metadata(&path).await {
                if let Ok(modified) = meta.modified() {
                    let age = std::time::SystemTime::now()
                        .duration_since(modified)
                        .unwrap_or_default();
                    if age < std::time::Duration::from_secs(600) {
                        // File is fresh and owned by another agent, skip.
                        return Ok(());
                    }
                }
            }
        }
    }

    fs::write(&path, content).await?;
    Ok(())
}
