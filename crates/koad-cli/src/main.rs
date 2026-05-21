#![allow(dead_code, unused_imports, clippy::type_complexity)]

mod cli;
mod handlers;
mod tui;
mod utils;

use anyhow::{Context, Result};
use clap::Parser;
use fred::interfaces::HashesInterface;
use koad_core::config::KoadConfig;
use koad_core::logging::init_logging;
use koad_core::session::AgentSession;
use koad_core::utils::redis::RedisClient;
use koad_core::db::KoadDB;
use std::env;
use std::path::PathBuf;

use crate::cli::{AgentAction, Cli, Commands, SystemAction, UpdatesAction, XpCommands};
// use crate::db::KoadDB;
use crate::handlers::boot::handle_boot_command;
use crate::handlers::bridge::handle_bridge_action;
use crate::handlers::fleet::handle_fleet_action;
use crate::handlers::intel::handle_intel_action;
use crate::handlers::status::{handle_doctor_command, handle_status_command};
use crate::handlers::system::handle_system_action;
use crate::handlers::xp::handle_xp_command;
use crate::utils::{detect_model_tier, feature_gate, pre_flight, PreFlightStatus};
use std::collections::HashMap;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = KoadConfig::load()?;
    let role = cli.role.clone();
    let _is_admin = role == "admin";

    // 1. Determine Agent Name
    let agent_name = match &cli.command {
        Commands::Boot { agent, .. } => agent.clone(),
        _ => config.get_agent_name(),
    };

    // 2. Logging
    let log_dir = Some(config.home.join("logs"));
    let _guard = init_logging(&agent_name, log_dir);

    // 3. Database
    let db = KoadDB::new(&config.get_db_path())?;

    // 4.1 Redis Configuration Hydration (Hot Config Override)
    let mut config = config;
    if config.get_redis_socket().exists() {
        use fred::interfaces::KeysInterface;
        use koad_core::utils::redis::RedisClient;

        if let Ok(client) = RedisClient::new(&config.home.to_string_lossy(), false).await {
            if let Ok(Some(json)) = client
                .pool
                .get::<Option<String>, _>(koad_core::constants::REDIS_KEY_CONFIG)
                .await
            {
                if let Ok(hot_config) = KoadConfig::from_json(&json) {
                    // Merge hot config: preserve local paths, take hot settings
                    let home = config.home.clone();
                    let extra = config.extra.clone();

                    config = hot_config;
                    config.home = home;
                    // Merge extra fields
                    for (k, v) in extra {
                        config.extra.entry(k).or_insert(v);
                    }
                }
            }
        }
    }

    // 4.2 Role Resolution (Auto-lookup for non-sovereign agents if default is used)
    let mut role = cli.role.clone();
    if role == "admin" {
        if let Commands::Boot { ref agent, .. } = cli.command {
            let is_sovereign = if let Some(id_config) = config.identities.get(agent) {
                let r = id_config.rank.to_lowercase();
                r == "admiral" || r == "captain"
            } else {
                false
            };

            if !is_sovereign {
                if let Ok(Some(db_role)) = db.get_primary_role(agent) {
                    role = db_role;
                }
            }
        }
    }
    let is_admin = role == "admin";

    // 4.3 Path-Aware Project Context
    let current_dir = env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    if let Some((_name, project)) = config.resolve_project_context(&current_dir) {
        env::set_var("GITHUB_OWNER", config.get_github_owner(Some(&project)));
        env::set_var("GITHUB_REPO", config.get_github_repo(Some(&project)));
    } else if current_dir.to_string_lossy().contains("skylinks") {
        env::set_var("GITHUB_OWNER", "Skylinks-Golf");
        // Only override GITHUB_REPO if we are actually inside an app directory
        if let Some(repo_name) = current_dir.file_name() {
            if current_dir.to_string_lossy().contains("/apps/") {
                env::set_var("GITHUB_REPO", repo_name);
            }
        }
    }

    // 5. Pre-Flight

    // 6. Command Routing
    match cli.command {
        Commands::Boot {
            agent,
            project,
            task,
            compact,
            budget,
            force,
            export_env,
        } => {
            // Pre-boot sovereign check
            let is_authorized = if let Some(id_config) = config.identities.get(&agent) {
                let r = id_config.rank.to_lowercase();
                r == "admiral" || r == "captain" || r == "officer" || r == "dood" || r == "admin"
            } else {
                false
            };

            if !is_authorized {
                anyhow::bail!(
                    "\x1b[31mREJECTION\x1b[0m: Agent '{}' is not a registered Sovereign KAI. \
                     Only Officers and Captains can be booted as primary ghosts.",
                    agent
                );
            }

            use crate::handlers::boot::BootOptions;
            handle_boot_command(
                BootOptions {
                    agent,
                    project,
                    task,
                    compact,
                    budget,
                    force,
                    role,
                    export_env,
                },
                &config,
            )
            .await?;
        }
        Commands::System { action } => match action {
            SystemAction::Import {
                source,
                format,
                delimiter,
                route,
                template,
                labels,
                dry_run,
            } => {
                crate::handlers::import::handle_import(
                    source, format, delimiter, route, template, labels, dry_run, &config, &db,
                )
                .await?;
            }
            _ => {
                handle_system_action(action, &config, &db, role, is_admin, &agent_name).await?;
            }
        },
        Commands::Intel { action } => {
            handle_intel_action(action, &config, &db, &agent_name).await?;
        }
        Commands::Fleet { action } => {
            handle_fleet_action(action, &config, &db).await?;
        }
        Commands::Bridge { action } => {
            handle_bridge_action(action, &config, &db).await?;
        }
        Commands::Signal { action } => {
            crate::handlers::signal::handle_signal_action(action, &config, &agent_name).await?
        }
        Commands::Guide { topic } => {
            crate::handlers::guide::handle_guide_action(topic, &config).await?
        }
        Commands::Xp { action } => {
            handle_xp_command(action, &config).await?;
        }
        Commands::Version => {
            println!("KoadOS CLI v{}", env!("CARGO_PKG_VERSION"));
            // Query Citadel version
            if let Ok(mut client) = koad_proto::citadel::v5::admin_client::AdminClient::connect(
                config.network.citadel_grpc_addr.clone(),
            )
            .await
            {
                let context = Some(crate::utils::get_trace_context(&agent_name, 3));
                if let Ok(resp) = client
                    .get_system_status(koad_proto::citadel::v5::SystemStatusRequest { context })
                    .await
                {
                    println!("KoadOS Citadel v{}", resp.into_inner().version);
                }
            }
        }
        Commands::Board { action } => {
            crate::handlers::board::handle_board(action, &config).await?;
        }
        Commands::Project { action } => {
            crate::handlers::project::handle_project(action, &config).await?;
        }
        Commands::Review { file } => {
            crate::handlers::review::handle_review(&file, &config).await?;
        }
        Commands::Status { json, full } => {
            if full && !json {
                let _ = crate::handlers::motd::show_motd(&agent_name, &config).await;
            }
            handle_status_command(json, full, false, &config, &db).await?;
        }
        Commands::Doctor { fix, gpu } => {
            handle_doctor_command(fix, gpu, &config, &db).await?;
        }
        Commands::Whoami => {
            crate::handlers::whoami::handle_whoami(&config, &db).await?;
        }
        Commands::Cognitive => {
            let session_id = std::env::var("KOAD_SESSION_ID").unwrap_or_default();
            let final_agent_name = if !session_id.is_empty() {
                let redis_client = RedisClient::new(&config.home.to_string_lossy(), false).await?;
                let session_key = format!("koad:session:{}", session_id);
                let res: Option<String> =
                    redis_client.pool.hget("koad:state", &session_key).await?;

                res.and_then(|data| {
                    serde_json::from_str::<koad_core::session::AgentSession>(&data).ok()
                })
                .map(|s| s.identity.name)
                .unwrap_or_else(|| agent_name.clone())
            } else {
                agent_name.clone()
            };

            crate::handlers::cognitive::handle_cognitive_check(&config, &db, &final_agent_name)
                .await?;
        }
        Commands::Map { action, verbose } => {
            crate::handlers::map::handle_map(action, verbose, &config, &db).await?;
        }
        Commands::Logout { session } => {
            crate::handlers::boot::handle_logout_command(session, &config).await?;
        }
        Commands::Agent { action } => {
            crate::handlers::agent::handle_agent_action(action, &config).await?;
        }
        Commands::Vault { action } => {
            crate::handlers::vault::handle_vault_action(action, &config).await?;
        }
        Commands::Updates { action } => {
            crate::handlers::updates::handle_updates_action(action, &config).await?;
        }
        Commands::Pulse {
            message,
            role,
            list,
        } => {
            if list {
                crate::handlers::pulse::handle_pulse_list(role, &config).await?;
            } else {
                let msg = message.unwrap_or_else(|| "Agent online.".to_string());
                crate::handlers::pulse::handle_pulse_add(msg, role, &config).await?;
            }
        }
        Commands::Sandbox {
            command,
            image,
            network,
            memory,
            podman,
        } => {
            crate::handlers::sandbox::handle_sandbox_run(command, image, network, memory, podman)
                .await?;
        }
        Commands::Deploy { action } => {
            crate::handlers::deploy::handle_deploy_action(action, &config).await?;
        }
        Commands::Saveup { full } => {
            handle_system_action(
                SystemAction::Save { full },
                &config,
                &db,
                role,
                is_admin,
                &agent_name,
            )
            .await?;
        }
    }

    Ok(())
}
