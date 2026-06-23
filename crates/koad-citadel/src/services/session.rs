//! Citadel Session Service
//!
//! Handles the agent lifecycle (leases, sessions, heartbeats) and enforces
//! the "One Body, One Ghost" policy.

use crate::auth::session_cache::{ActiveSessions, SessionRecord};
use crate::state::bay_store::BayStore;
use crate::state::docking::DockingState;
use crate::state::storage_bridge::CitadelStorageBridge;
use chrono::Utc;
use fred::interfaces::HashesInterface;
use koad_core::config::KoadConfig;
use koad_core::hierarchy::HierarchyManager;
use koad_core::signal::SignalCorps;
use koad_proto::citadel::v5::citadel_session_server::CitadelSession;
use koad_proto::citadel::v5::*;
use serde_json::json;
use std::sync::Arc;
use parking_lot::Mutex;
use tonic::{Request, Response, Status};
use tracing::{info, warn};

/// Service implementation for the `CitadelSession` gRPC interface.
#[derive(Clone)]
pub struct CitadelSessionService {
    signal_corps: Arc<SignalCorps>,
    storage: Arc<CitadelStorageBridge>,
    bay_store: Arc<BayStore>,
    hierarchy: Arc<HierarchyManager>,
    config: KoadConfig,
    sessions: ActiveSessions,
    lease_duration_secs: u64,
}

impl CitadelSessionService {
    /// Creates a new `CitadelSessionService`.
    pub fn new(
        signal_corps: Arc<SignalCorps>,
        storage: Arc<CitadelStorageBridge>,
        bay_store: Arc<BayStore>,
        hierarchy: Arc<HierarchyManager>,
        config: KoadConfig,
        lease_duration_secs: u64,
    ) -> Self {
        Self {
            signal_corps,
            storage,
            bay_store,
            hierarchy,
            config,
            sessions: Arc::new(Mutex::new(std::collections::HashMap::new())),
            lease_duration_secs,
        }
    }

    /// Provides a handle to the active sessions map for monitoring or reaper tasks.
    pub fn sessions_handle(&self) -> ActiveSessions {
        self.sessions.clone()
    }

    /// The automated reaper task that transitions stale sessions to `DARK` or `TEARDOWN`.
    pub async fn reap(&self, dark_timeout_secs: u64, purge_timeout_secs: u64) {
        let now = Utc::now();
        let mut to_teardown = Vec::new();
        let mut to_dark = Vec::new();

        {
            let mut sessions = self.sessions.lock();
            for (sid, record) in sessions.iter_mut() {
                let elapsed = (now - record.last_heartbeat).num_seconds() as u64;

                if elapsed > purge_timeout_secs && record.state != DockingState::Teardown {
                    warn!(
                        "Reaper: Session '{}' ({}) exceeded purge timeout ({}s). Teardown.",
                        sid, record.agent_name, elapsed
                    );
                    record.state = DockingState::Teardown;
                    to_teardown.push((sid.clone(), record.agent_name.clone()));
                } else if elapsed > dark_timeout_secs
                    && record.state.is_alive()
                    && record.state != DockingState::Dark
                {
                    info!(
                        "Reaper: Session '{}' ({}) missed heartbeat ({}s). Marking DARK.",
                        sid, record.agent_name, elapsed
                    );
                    record.state = DockingState::Dark;
                    to_dark.push((sid.clone(), record.agent_name.clone(), elapsed));
                }
            }
        }

        for (sid, agent_name, elapsed) in to_dark {
            let _ = self
                .bay_store
                .log_state_transition(
                    &agent_name,
                    &sid,
                    "DARK",
                    Some(&format!("Heartbeat missed for {}s", elapsed)),
                )
                .await;
        }

        for (sid, agent_name) in to_teardown {
            let record_opt = {
                let mut sessions = self.sessions.lock();
                sessions.remove(&sid)
            };

            if let Some(record) = record_opt {
                let _ = self
                    .bay_store
                    .record_session_end(
                        &agent_name,
                        &sid,
                        record.last_heartbeat.timestamp(),
                        None,
                        "TEARDOWN",
                    )
                    .await;
                let lease_key = format!("koad:kai:{}:lease", agent_name);
                let session_key = format!("koad:session:{}", sid);
                let _: Result<(), _> = self
                    .storage
                    .redis
                    .pool
                    .hdel("koad:state", vec![lease_key, session_key])
                    .await;
            }
        }
    }
}

#[tonic::async_trait]
impl CitadelSession for CitadelSessionService {
    async fn create_lease(
        &self,
        request: Request<LeaseRequest>,
    ) -> Result<Response<LeaseResponse>, Status> {
        let req = request.into_inner();
        let agent_name = &req.agent_name;
        let force = req.force;
        let body_id = if req.body_id.is_empty() {
            uuid::Uuid::new_v4().to_string()
        } else {
            req.body_id.clone()
        };

        info!(agent = %agent_name, force = %force, "CreateLease: Requesting lease");

        let project_path = std::path::Path::new(&req.project_root);
        let resolved_level = self.hierarchy.resolve_level(project_path);

        let agent_rank = self
            .config
            .identities
            .get(&agent_name.to_lowercase())
            .map(|id| capitalize_first(&id.rank))
            .unwrap_or_else(|| "Crew".to_string());

        if !self.hierarchy.validate_access(&agent_rank, resolved_level) {
            return Err(Status::permission_denied(format!(
                "RANK_INSUFFICIENT: Agent '{}' (Rank: {}) cannot operate at Level {:?}",
                agent_name, agent_rank, resolved_level
            )));
        }

        let lease_key = format!("koad:kai:{}:lease", agent_name);
        let existing: Option<String> = self
            .storage
            .redis
            .pool
            .hget("koad:state", &lease_key)
            .await
            .map_err(|e| Status::internal(format!("Redis error: {}", e)))?;

        if let Some(lease_data) = existing {
            if let Ok(lease) = serde_json::from_str::<serde_json::Value>(&lease_data) {
                let is_live = lease["expires_at"]
                    .as_str()
                    .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    .map(|exp| exp.with_timezone(&Utc) > Utc::now())
                    .unwrap_or(false);

                if is_live && !force {
                    return Err(Status::already_exists(format!(
                        "SOVEREIGN_OCCUPIED: {} is active.",
                        agent_name
                    )));
                }

                if is_live && force {
                    let old_sid = lease["session_id"].as_str().unwrap_or("");
                    let old_session_key = format!("koad:session:{}", old_sid);
                    let _: Result<(), _> = self
                        .storage
                        .redis
                        .pool
                        .hdel("koad:state", vec![lease_key.clone(), old_session_key])
                        .await;
                    {
                        let mut sessions = self.sessions.lock();
                        sessions.remove(old_sid);
                    }
                }
            }
        }

        let session_id = format!(
            "SID-{}-{}",
            agent_name.to_lowercase(),
            &uuid::Uuid::new_v4().to_string()[..8]
        );
        let token = uuid::Uuid::new_v4().to_string();
        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(self.lease_duration_secs as i64);
        let level_str = format!("{:?}", resolved_level);

        let lease_value = json!({
            "session_id": session_id,
            "body_id": body_id,
            "agent_name": agent_name,
            "token": token,
            "level": level_str,
            "created_at": now.to_rfc3339(),
            "expires_at": expires_at.to_rfc3339(),
        });

        let lease_json = lease_value.to_string();
        let session_key = format!("koad:session:{}", session_id);

        let _: () = self
            .storage
            .redis
            .pool
            .hset("koad:state", (lease_key.clone(), &lease_json))
            .await
            .map_err(|e: fred::error::RedisError| Status::internal(e.to_string()))?;
        let _: () = self
            .storage
            .redis
            .pool
            .hset("koad:state", (session_key, &lease_json))
            .await
            .map_err(|e: fred::error::RedisError| Status::internal(e.to_string()))?;

        {
            let mut sessions = self.sessions.lock();
            sessions.insert(
                session_id.clone(),
                SessionRecord {
                    agent_name: agent_name.clone(),
                    state: DockingState::Active,
                    last_heartbeat: now,
                    body_id: body_id.clone(),
                    session_token: token.clone(),
                    level: level_str,
                },
            );
        }

        info!(agent = %agent_name, session_id = %session_id, level = ?resolved_level, "CreateLease: Lease granted");

        if let Some(metrics) = req.metrics {
            info!(
                "Telemetry [BOOT]: agent={}, tokens_out={}, reason='hydration'",
                agent_name, metrics.output_tokens
            );
        }

        Ok(Response::new(LeaseResponse {
            session_id,
            token,
            context: Some(TraceContext {
                trace_id: "BOOT".to_string(),
                origin: "Citadel".to_string(),
                actor: "citadel".to_string(),
                timestamp: Some(prost_types::Timestamp {
                    seconds: now.timestamp(),
                    nanos: 0,
                }),
                level: resolved_level as i32,
            }),
        }))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        let sid = &req.session_id;
        let now = Utc::now();

        {
            let mut sessions = self.sessions.lock();
            if let Some(record) = sessions.get_mut(sid) {
                record.last_heartbeat = now;
                record.state = DockingState::Active;

                if let Some(metrics) = req.metrics {
                    info!(
                        "Telemetry [{}]: tokens_in={}, tokens_out={}",
                        sid, metrics.input_tokens, metrics.output_tokens
                    );
                }

                Ok(Response::new(StatusResponse {
                    success: true,
                    message: "Heartbeat acknowledged".to_string(),
                    context: None,
                }))
            } else {
                Err(Status::not_found(format!("Session {} not found", sid)))
            }
        }
    }

    async fn close_session(
        &self,
        request: Request<CloseRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        let sid = &req.session_id;

        // Broadcast session_closed event for CASS EoW Pipeline
        let record_opt = {
            let mut sessions = self.sessions.lock();
            sessions.remove(sid)
        };

        if let Some(record) = record_opt {
            let _ = self.signal_corps.broadcast(
                "system",
                &format!("{{\"event_type\": \"session_closed\", \"session_id\": \"{}\", \"agent_name\": \"{}\"}}", sid, record.agent_name),
                "EOW-TRIGGER",
                "citadel",
            ).await;

            let lease_key = format!("koad:kai:{}:lease", record.agent_name);
            let session_key = format!("koad:session:{}", sid);
            let _: Result<(), _> = self
                .storage
                .redis
                .pool
                .hdel("koad:state", vec![lease_key, session_key])
                .await;

            info!("CloseSession: Session {} terminated.", sid);
            Ok(Response::new(StatusResponse {
                success: true,
                message: "Session closed".to_string(),
                context: None,
            }))
        } else {
            Err(Status::not_found(format!("Session {} not found", sid)))
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
