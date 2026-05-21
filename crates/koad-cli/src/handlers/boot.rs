use crate::handlers::motd::show_motd;
use crate::utils::errors::{map_connect_err, map_status_err};
use anyhow::{Context, Result};
use fred::interfaces::HashesInterface;
use koad_core::config::KoadConfig;
use koad_core::hierarchy::HierarchyManager;
use koad_core::utils::redis::RedisClient;
use koad_proto::cass::v1::hydration_service_client::HydrationServiceClient;
use koad_proto::cass::v1::HydrationRequest;
use koad_proto::citadel::v5::citadel_session_client::CitadelSessionClient;
use koad_proto::citadel::v5::{CloseRequest, LeaseRequest, TurnMetrics};
use std::env;

/// Options for booting an agent session.
pub struct BootOptions {
    pub agent: String,
    pub project: bool,
    pub task: Option<String>,
    pub compact: bool,
    pub budget: u32,
    pub force: bool,
    pub role: String,
    pub export_env: bool,
}

pub async fn handle_boot_command(opts: BootOptions, config: &KoadConfig) -> Result<()> {
    let agent = opts.agent;
    let force = opts.force;
    let budget = opts.budget;
    let compact = opts.compact;
    let project = opts.project;
    let role = opts.role;
    // --- [Laws of Consciousness: Occupancy Check] ---
    if let Ok(existing_sid) = env::var("KOAD_SESSION_ID") {
        if !existing_sid.is_empty() {
            let rc = RedisClient::new(&config.home.to_string_lossy(), false).await?;
            let session_key = format!("koad:session:{}", existing_sid);
            let val: Option<String> = rc
                .pool
                .hget("koad:state", &session_key)
                .await
                .unwrap_or(None);

            if val.is_some() {
                anyhow::bail!(
                    "\x1b[31mCONSCIOUSNESS_COLLISION\x1b[0m: Session {} is already active in this Body. \
                     Run `koad logout` to untether before booting a new agent.",
                    existing_sid
                );
            }
        }
    }

    let current_dir = env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
    let body_id = env::var("KOAD_BODY_ID").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());

    // 1. Resolve Level
    let hierarchy = HierarchyManager::new(config.clone());
    let level = if project {
        hierarchy.resolve_level(&current_dir) as i32
    } else {
        0 // Level Unspecified
    };

    // 2. Pre-boot Hydration (CASS)
    // We do this BEFORE the lease so we can report the token cost in the CreateLease call.
    let context_file = config.home.join("current_context.md");
    let mut is_degraded = false;
    let mut estimated_tokens = 0;

    let cass_connect_result =
        HydrationServiceClient::connect(config.network.cass_grpc_addr.clone()).await;
    match cass_connect_result {
        Ok(mut cass) => {
            match cass
                .hydrate(HydrationRequest {
                    agent_name: agent.clone(),
                    project_root: current_dir.to_string_lossy().to_string(),
                    level,
                    token_budget: budget,
                    task_id: opts.task.unwrap_or_default(),
                })
                .await
            {
                Ok(hydration_resp) => {
                    let packet = hydration_resp.into_inner();
                    estimated_tokens = packet.estimated_tokens;
                    tokio::fs::write(&context_file, packet.markdown_packet.as_bytes()).await?;
                }
                Err(e) => {
                    is_degraded = true;
                    eprintln!("{}", map_status_err("KoadOS CASS", e));
                    tokio::fs::write(&context_file, b"# [SYSTEM DEGRADED: CASS OFFLINE]\nCould not fetch Temporal Context Hydration.").await?;
                }
            }
        }
        Err(e) => {
            is_degraded = true;
            eprintln!(
                "{}",
                map_connect_err("KoadOS CASS", &config.network.cass_grpc_addr, e)
            );
            tokio::fs::write(
                &context_file,
                b"# [SYSTEM DEGRADED: CASS OFFLINE]\nCould not connect to CASS gRPC service.",
            )
            .await?;
        }
    }

    // 3. Citadel Handshake (Lease + Telemetry)
    let mut session_id = format!("local-fallback-{}", uuid::Uuid::new_v4());
    let citadel_connect_result =
        CitadelSessionClient::connect(config.network.citadel_grpc_addr.clone()).await;

    match citadel_connect_result {
        Ok(mut citadel) => {
            let context = Some(crate::utils::get_trace_context(&agent, 3));
            match citadel
                .create_lease(LeaseRequest {
                    context: context.clone(),
                    agent_name: agent.clone(),
                    project_root: current_dir.to_string_lossy().to_string(),
                    force,
                    body_id: body_id.clone(),
                    driver_id: "gemini-cli".to_string(),
                    metrics: Some(TurnMetrics {
                        input_tokens: 0,
                        output_tokens: estimated_tokens,
                        thinking_tokens: 0,
                        tool_calls: 0,
                    }),
                })
                .await
            {
                Ok(lease_resp) => {
                    session_id = lease_resp.into_inner().session_id;
                }
                Err(e) => {
                    is_degraded = true;
                    eprintln!("{}", map_status_err("KoadOS Citadel", e));
                }
            }
        }
        Err(e) => {
            is_degraded = true;
            eprintln!(
                "{}",
                map_connect_err("KoadOS Citadel", &config.network.citadel_grpc_addr, e)
            );
        }
    }

    if is_degraded {
        eprintln!("\x1b[1;31mWARNING: You are operating in DEGRADED MODE. Governance, shared memory, and telemetry are offline.\x1b[0m");
    }

    // 4. Environment Export (The Link)
    if compact {
        println!(
            "I:{}|R:{}|S:{}|X:export KOAD_SESSION_ID={} KOAD_BODY_ID={} KOAD_CONTEXT_FILE={}",
            agent,
            role,
            session_id,
            session_id,
            body_id,
            context_file.display()
        );
    } else if opts.export_env {
        // Bare export lines only — safe for `eval $(koad boot --agent X --export-env)`
        println!("export KOAD_SESSION_ID=\"{}\"", session_id);
        println!("export KOAD_BODY_ID=\"{}\"", body_id);
        println!("export KOAD_CONTEXT_FILE=\"{}\"", context_file.display());
        println!("export KOAD_AGENT_NAME=\"{}\"", agent);
    } else {
        show_motd(&agent, config).await?;

        // Output the actual eval strings (hidden but necessary for eval $(...))
        println!("export KOAD_SESSION_ID=\"{}\"", session_id);
        println!("export KOAD_BODY_ID=\"{}\"", body_id);
        println!("export KOAD_CONTEXT_FILE=\"{}\"", context_file.display());
        println!("export KOAD_AGENT_NAME=\"{}\"", agent);
    }

    Ok(())
}

pub async fn handle_logout_command(session: Option<String>, config: &KoadConfig) -> Result<()> {
    let session_id = session
        .or_else(|| env::var("KOAD_SESSION_ID").ok())
        .context("No active session ID found.")?;

    let agent_name = env::var("KOAD_AGENT_NAME").unwrap_or_else(|_| "unknown".to_string());
    let context = Some(crate::utils::get_trace_context(&agent_name, 3));

    let mut client =
        CitadelSessionClient::connect(config.network.citadel_grpc_addr.clone()).await?;

    client
        .close_session(CloseRequest {
            context,
            session_id: session_id.clone(),
            summary_path: String::new(),
        })
        .await?;

    println!("\x1b[32m[OK]\x1b[0m Session {} untethered.", session_id);
    Ok(())
}
