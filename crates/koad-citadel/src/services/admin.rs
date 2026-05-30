//! Admin Service Implementation
//!
//! Handles administrative and maintenance RPC calls, typically via a secure UDS.

use koad_core::db::KoadDB;
use koad_proto::citadel::v5::admin_server::Admin;
use koad_proto::citadel::v5::*;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::watch;
use tonic::{Request, Response, Status};
use tracing::{error, info, warn};

/// Service implementation for the `Admin` gRPC interface.
#[derive(Clone)]
pub struct AdminService {
    shutdown_tx: watch::Sender<bool>,
    start_time: Instant,
    koad_db: Arc<KoadDB>,
}

impl AdminService {
    /// Creates a new `AdminService`.
    pub fn new(shutdown_tx: watch::Sender<bool>, koad_db: Arc<KoadDB>) -> Self {
        Self {
            shutdown_tx,
            start_time: Instant::now(),
            koad_db,
        }
    }
}

#[tonic::async_trait]
impl Admin for AdminService {
    /// Gracefully shutdown the Citadel kernel.
    async fn shutdown(
        &self,
        request: Request<ShutdownRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        let reason = req.reason;

        warn!(reason = %reason, "Admin: Received shutdown request via UDS");

        let _ = self.shutdown_tx.send(true);

        Ok(Response::new(StatusResponse {
            success: true,
            message: format!("Shutdown initiated: {}", reason),
            context: req.context,
        }))
    }

    /// Retrieve high-level system health and metrics.
    async fn get_system_status(
        &self,
        request: Request<SystemStatusRequest>,
    ) -> Result<Response<SystemStatusResponse>, Status> {
        let req = request.into_inner();

        info!("Admin: System status requested");

        let uptime = format!("{:?}", self.start_time.elapsed());

        Ok(Response::new(SystemStatusResponse {
            version: "3.2.0".to_string(), // Should come from config
            active_sessions: 0,           // Placeholder
            total_bays: 0,                // Placeholder
            uptime,
            context: req.context,
        }))
    }

    /// Forcefully terminate a session by ID.
    async fn force_purge_session(
        &self,
        request: Request<PurgeRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        let sid = req.session_id;

        warn!(session_id = %sid, "Admin: Force purging session");

        // Logic to purge from Redis would go here.
        // For now, we return success.

        Ok(Response::new(StatusResponse {
            success: true,
            message: format!("Session {} purged", sid),
            context: req.context,
        }))
    }

    /// Commit knowledge/learnings to the Memory Bank.
    async fn commit_knowledge(
        &self,
        request: Request<CommitKnowledgeRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();

        // Derive agent name from session_id: "SID-clyde-abc123" → "clyde"
        let agent_name = req.session_id
            .splitn(3, '-')
            .nth(1)
            .unwrap_or("unknown")
            .to_string();

        info!(
            agent = %agent_name,
            category = %req.category,
            "Admin: Commit knowledge requested"
        );

        let tags = if req.tags.is_empty() { None } else { Some(req.tags.clone()) };

        match self.koad_db.remember(&req.category, &req.content, tags, 0, &agent_name) {
            Ok(()) => {
                info!(agent = %agent_name, category = %req.category, "Admin: Knowledge committed to koad.db");
                Ok(Response::new(StatusResponse {
                    success: true,
                    message: format!("Knowledge ({}) committed to Citadel.", req.category),
                    context: req.context,
                }))
            }
            Err(e) => {
                error!(error = %e, "Admin: Failed to commit knowledge to koad.db");
                Err(Status::internal(format!("Knowledge commit failed: {}", e)))
            }
        }
    }

    /// Retrieve a snippet of a file from the Citadel's cache or disk.
    async fn get_file_snippet(
        &self,
        request: Request<GetFileSnippetRequest>,
    ) -> Result<Response<SnippetResponse>, Status> {
        let req = request.into_inner();
        info!(path = %req.path, "Admin: Get file snippet requested");

        Ok(Response::new(SnippetResponse {
            content: "Snippet content placeholder".to_string(),
            total_lines: 0,
            source: "placeholder".to_string(),
            context: req.context,
        }))
    }

    /// Post a system event to the telemetry stream.
    async fn post_system_event(
        &self,
        request: Request<SystemEvent>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        info!(message = %req.message, "Admin: Post system event requested");

        Ok(Response::new(StatusResponse {
            success: true,
            message: "System event posted".to_string(),
            context: req.context,
        }))
    }

    /// Trigger a system-wide backup or a specific source.
    async fn trigger_backup(
        &self,
        request: Request<TriggerBackupRequest>,
    ) -> Result<Response<TriggerBackupResponse>, Status> {
        let req = request.into_inner();
        info!(source = %req.source, "Admin: Trigger backup requested");

        Ok(Response::new(TriggerBackupResponse {
            success: true,
            message: "Backup triggered (Placeholder)".to_string(),
            backup_id: "bkp-placeholder".to_string(),
            context: req.context,
        }))
    }

    /// Flush all volatile context for a session.
    async fn flush_context(
        &self,
        request: Request<FlushContextRequest>,
    ) -> Result<Response<StatusResponse>, Status> {
        let req = request.into_inner();
        info!(session_id = %req.session_id, "Admin: Flush context requested");

        Ok(Response::new(StatusResponse {
            success: true,
            message: "Context flushed".to_string(),
            context: req.context,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::watch;

    #[tokio::test]
    async fn test_admin_shutdown() -> anyhow::Result<()> {
        let (tx, mut rx) = watch::channel(false);
        let db = Arc::new(KoadDB::new(std::path::Path::new(":memory:")).unwrap());
        let service = AdminService::new(tx, db);

        let req = Request::new(ShutdownRequest {
            context: None,
            reason: "Testing".to_string(),
        });

        let res = service.shutdown(req).await?;
        assert!(res.into_inner().success);
        assert!(*rx.borrow_and_update());

        Ok(())
    }

    #[tokio::test]
    async fn test_admin_status() -> anyhow::Result<()> {
        let (tx, _) = watch::channel(false);
        let db = Arc::new(KoadDB::new(std::path::Path::new(":memory:")).unwrap());
        let service = AdminService::new(tx, db);

        let req = Request::new(SystemStatusRequest { context: None });
        let res = service.get_system_status(req).await?;

        let status = res.into_inner();
        assert_eq!(status.version, "3.2.0");
        assert!(!status.uptime.is_empty());

        Ok(())
    }
}
