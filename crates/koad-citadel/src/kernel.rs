//! Citadel Kernel
//!
//! The Kernel is the central orchestration engine of the Citadel, managing the lifecycle
//! of gRPC services, background tasks (reapers, drain loops), and network listeners.

use crate::auth::interceptor::build_citadel_interceptor;
use crate::services::admin::AdminService;
use crate::services::bay::PersonalBayService;
use crate::services::sector::SectorService;
use crate::services::session::CitadelSessionService;
use crate::services::signal::SignalService;
use crate::services::xp::CitadelXpService;
use crate::signal_corps::quota::QuotaValidator;
use crate::state::bay_store::BayStore;
use crate::state::storage_bridge::CitadelStorageBridge;
use crate::workspace::manager::WorkspaceManager;
use koad_core::hierarchy::HierarchyManager;
use koad_core::signal::SignalCorps;
use koad_core::storage::StorageBridge;

use koad_core::config::KoadConfig;
use koad_core::utils::redis::RedisClient;
use koad_proto::citadel::v5::admin_server::AdminServer;
use koad_proto::citadel::v5::citadel_session_server::CitadelSessionServer;
use koad_proto::citadel::v5::personal_bay_server::PersonalBayServer;
use koad_proto::citadel::v5::sector_server::SectorServer;
use koad_proto::citadel::v5::signal_server::SignalServer;
use koad_proto::citadel::v5::xp_service_server::XpServiceServer;
use koad_sandbox::Sandbox;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use tokio_stream::wrappers::UnixListenerStream;
use tonic::transport::Server;
use tracing::{error, info};
/// The central nervous system of the Citadel.
pub struct Kernel {
    shutdown_tx: watch::Sender<bool>,
    storage: Arc<CitadelStorageBridge>,
    admin_uds_path: Option<PathBuf>,
}

impl Kernel {
    /// Initiates a graceful shutdown of all kernel services and listeners.
    pub async fn shutdown(self) {
        info!("Kernel: Initiating graceful shutdown...");

        // 1. Notify all tasks to stop
        let _ = self.shutdown_tx.send(true);

        // 2. Wait for background tasks (reaper, drain loop, servers) to settle
        tokio::time::sleep(Duration::from_millis(800)).await;

        // 3. Final Storage Drain (L1 -> L2)
        info!("Kernel: Finalizing neuronal flush (L1 -> L2 drain)...");
        if let Err(e) = self.storage.drain_all().await {
            error!("Kernel: Final drain failed: {}", e);
        }

        // 4. Cleanup Sockets
        if let Some(path) = self.admin_uds_path {
            if path.exists() {
                let _ = std::fs::remove_file(&path);
                info!("Kernel: Admin UDS socket removed.");
            }
        }

        info!("Kernel: Shutdown complete.");
    }
}

/// Builder for configuring and constructing a [`Kernel`] instance.
#[derive(Default)]
pub struct KernelBuilder {
    home_dir: Option<PathBuf>,
    tcp_addr: Option<String>,
    uds_path: Option<PathBuf>,
    admin_uds_path: Option<PathBuf>,
    config: Option<KoadConfig>,
}

impl KernelBuilder {
    /// Creates a new `KernelBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the home directory for the Citadel.
    pub fn with_home(mut self, path: PathBuf) -> Self {
        self.home_dir = Some(path);
        self
    }

    /// Sets the TCP address for the primary gRPC listener.
    pub fn with_tcp(mut self, addr: &str) -> Self {
        self.tcp_addr = Some(addr.to_string());
        self
    }

    /// Sets the UDS path for the primary gRPC listener (Optional).
    pub fn with_uds(mut self, path: PathBuf) -> Self {
        self.uds_path = Some(path);
        self
    }

    /// Sets the UDS path for the administrative listener (Emergency Override).
    pub fn with_admin_uds(mut self, path: PathBuf) -> Self {
        self.admin_uds_path = Some(path);
        self
    }

    /// Sets the `KoadConfig` for the kernel.
    pub fn with_config(mut self, config: KoadConfig) -> Self {
        self.config = Some(config);
        self
    }

    /// Starts the Citadel kernel, initializing all services and listeners.
    ///
    /// # Errors
    /// Returns an error if any required parameters are missing or if service
    /// initialization fails.
    pub async fn start(self) -> anyhow::Result<Kernel> {
        let home_dir = self
            .home_dir
            .ok_or_else(|| anyhow::anyhow!("Home directory not specified"))?;
        let config = self
            .config
            .ok_or_else(|| anyhow::anyhow!("Config not specified"))?;
        let tcp_addr = self
            .tcp_addr
            .ok_or_else(|| anyhow::anyhow!("TCP address not specified"))?;

        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        info!("Kernel: Initializing at {}...", home_dir.display());

        let redis = Arc::new(RedisClient::new(&home_dir.to_string_lossy(), true).await?);
        let db_path = home_dir.join("data/db/citadel.db");
        let storage = Arc::new(CitadelStorageBridge::new(
            redis.clone(),
            &db_path.to_string_lossy(),
            config.storage.drain_interval_secs,
        )?);

        storage.enable_keyspace_notifications().await?;
        storage.hydrate_all().await?;

        let hierarchy = Arc::new(HierarchyManager::new(config.clone()));
        let bays_path = home_dir.join("agents/bays");
        let bay_store = Arc::new(BayStore::new(bays_path));
        let identities_dir = home_dir.join("config/identities");
        bay_store.auto_provision_all(&identities_dir).await?;

        let signal_corps = Arc::new(SignalCorps::new(redis.clone(), "koad:stream:", 1000));
        let quota = Arc::new(QuotaValidator::new(redis.clone(), 60, 60));
        let workspace_mgr = Arc::new(WorkspaceManager::new(
            home_dir.join("workspaces"),
            home_dir.clone(),
        ));
        let sandbox = Arc::new(Sandbox::new(config.clone()));

        let session_svc_impl = CitadelSessionService::new(
            signal_corps.clone(),
            storage.clone(),
            bay_store.clone(),
            hierarchy.clone(),
            config.clone(),
            config.sessions.lease_duration_secs,
        );
        let bay_svc_impl = PersonalBayService::new(bay_store.clone(), workspace_mgr.clone());
        let sector_svc_impl = SectorService::new(redis.clone(), sandbox.clone());
        let signal_svc_impl = SignalService::new(signal_corps.clone(), quota.clone());
        let admin_svc_impl = AdminService::new(shutdown_tx.clone(), config.network.cass_grpc_addr.clone());
        let xp_svc_impl = CitadelXpService::new(storage.sqlite.clone(), config.clone()).await?;

        let drain_storage = storage.clone();
        let mut rx_drain = shutdown_rx.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = drain_storage.start_drain_loop() => {},
                _ = rx_drain.changed() => { info!("Kernel: Drain loop stopping."); }
            }
        });

        let reaper_session = Arc::new(session_svc_impl.clone());
        let reaper_clone = reaper_session.clone();
        let dark_timeout = config.sessions.dark_timeout_secs;
        let purge_timeout = config.sessions.purge_timeout_secs;
        let reaper_interval = Duration::from_secs(config.sessions.reaper_interval_secs);
        let mut rx_reaper = shutdown_rx.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(reaper_interval) => { reaper_clone.reap(dark_timeout, purge_timeout).await; }
                    _ = rx_reaper.changed() => { info!("Kernel: Reaper stopping."); break; }
                }
            }
        });

        // 1. Standard TCP Listener (with Auth Interceptor)
        let tcp_addr_parsed: std::net::SocketAddr = tcp_addr.parse()?;
        let auth_interceptor = build_citadel_interceptor(session_svc_impl.sessions_handle());

        let tcp_router = Server::builder()
            .add_service(CitadelSessionServer::with_interceptor(
                session_svc_impl.clone(),
                auth_interceptor.clone(),
            ))
            .add_service(PersonalBayServer::with_interceptor(
                bay_svc_impl.clone(),
                auth_interceptor.clone(),
            ))
            .add_service(SectorServer::with_interceptor(
                sector_svc_impl.clone(),
                auth_interceptor.clone(),
            ))
            .add_service(SignalServer::with_interceptor(
                signal_svc_impl.clone(),
                auth_interceptor,
            ))
            .add_service(XpServiceServer::new(xp_svc_impl.clone()))
            .add_service(AdminServer::new(admin_svc_impl.clone()));

        let mut rx_tcp = shutdown_rx.clone();
        tokio::spawn(async move {
            info!("Kernel: TCP listener active at {}", tcp_addr_parsed);
            if let Err(e) = tcp_router
                .serve_with_shutdown(tcp_addr_parsed, async move {
                    let _ = rx_tcp.changed().await;
                })
                .await
            {
                error!("Kernel: TCP listener error: {}", e);
            }
        });

        // 2. Admin UDS Listener (Emergency Bypass)
        if let Some(ref admin_uds_path) = self.admin_uds_path {
            // Remove existing socket if any
            if admin_uds_path.exists() {
                let _ = std::fs::remove_file(admin_uds_path);
            }

            let uds = tokio::net::UnixListener::bind(admin_uds_path)?;
            let uds_stream = UnixListenerStream::new(uds);

            let admin_router = Server::builder()
                .add_service(AdminServer::new(admin_svc_impl))
                // Also serve core services on UDS without interceptor for emergency maintenance
                .add_service(CitadelSessionServer::new(session_svc_impl))
                .add_service(SectorServer::new(sector_svc_impl))
                .add_service(XpServiceServer::new(xp_svc_impl));

            let mut rx_admin = shutdown_rx.clone();
            let admin_path_clone = admin_uds_path.clone();
            tokio::spawn(async move {
                info!(
                    "Kernel: Admin UDS listener active at {}",
                    admin_path_clone.display()
                );
                if let Err(e) = admin_router
                    .serve_with_incoming_shutdown(uds_stream, async move {
                        let _ = rx_admin.changed().await;
                    })
                    .await
                {
                    error!("Kernel: Admin UDS listener error: {}", e);
                }
            });
        }

        Ok(Kernel {
            shutdown_tx,
            storage,
            admin_uds_path: self.admin_uds_path,
        })
    }
}
