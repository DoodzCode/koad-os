//! CASS Binary Entry Point

use anyhow::Result;
use koad_bridge_notion::NotionClient;
use koad_cass::services::eow::EndOfWatchPipeline;
use koad_cass::services::hydration::CassHydrationService;
use koad_cass::services::memory::CassMemoryService;
use koad_cass::services::pulse::CassPulseService;
use koad_cass::services::stream::CassStreamService;
use koad_cass::services::symbol::CassSymbolService;
use koad_cass::services::tool_registry::CassToolRegistryService;
use koad_cass::storage::{QdrantTier, RedisTier, SqliteTier, TieredStorage};
use koad_codegraph::CodeGraph;
use koad_core::config::KoadConfig;
use koad_core::hierarchy::HierarchyManager;
use koad_core::signal::SignalCorps;
use koad_core::utils::redis::RedisClient;
use koad_intelligence::router::InferenceRouter;
use koad_plugins::registry::PluginRegistry;

use koad_proto::cass::v1::hydration_service_server::HydrationServiceServer;
use koad_proto::cass::v1::memory_service_server::MemoryServiceServer;
use koad_proto::cass::v1::pulse_service_server::PulseServiceServer;
use koad_proto::cass::v1::stream_service_server::StreamServiceServer;
use koad_proto::cass::v1::symbol_service_server::SymbolServiceServer;
use koad_proto::cass::v1::tool_registry_service_server::ToolRegistryServiceServer;

use std::sync::Arc;
use tonic::transport::Server;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("CASS: Initializing Support Systems...");

    let config = KoadConfig::load()?;
    let redis = Arc::new(RedisClient::new(&config.home.to_string_lossy(), true).await?);
    let sqlite = Arc::new(SqliteTier::new(
        &config.home.join("data/db/cass.db").to_string_lossy(),
    )?);
    let redis_tier = Arc::new(RedisTier::new(redis.pool.clone()));

    let intelligence = Arc::new(InferenceRouter::new_default()?);

    let qdrant_url = std::env::var("KOADOS_URL_QDRANT")
        .or_else(|_| std::env::var("QDRANT_URL"))
        .unwrap_or_else(|_| "http://127.0.0.1:6334".to_string());

    let qdrant = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        QdrantTier::new(&qdrant_url, Some(Arc::clone(&intelligence))),
    )
    .await
    {
        Ok(Ok(q)) => {
            tracing::info!("Qdrant L3: ONLINE ({})", qdrant_url);
            Arc::new(q)
        }
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "Qdrant L3: OFFLINE ({}) — starting in degraded mode (L1+L2 only)", qdrant_url);
            Arc::new(QdrantTier::new_offline())
        }
        Err(_) => {
            tracing::warn!("Qdrant L3: TIMEOUT ({}) — starting in degraded mode (L1+L2 only)", qdrant_url);
            Arc::new(QdrantTier::new_offline())
        }
    };
    let storage = Arc::new(TieredStorage::new(Arc::clone(&redis_tier), sqlite, qdrant));
    let hierarchy = Arc::new(HierarchyManager::new(config.clone()));
    let signal_corps = Arc::new(SignalCorps::new(redis.clone(), "koad:stream:", 1000));
    let codegraph = Arc::new(CodeGraph::new(&config.home.join("data/db/codegraph.db"))?);
    let plugin_registry = PluginRegistry::new()?;

    let notion_key = std::env::var("KOADOS_PAT_NOTION_MAIN").unwrap_or_default();
    let notion_client = Arc::new(NotionClient::new(notion_key)?);
    let stream_db = config
        .integrations
        .notion
        .as_ref()
        .and_then(|n| n.index.get("stream"))
        .cloned()
        .unwrap_or_default();

    // Services
    let memory_svc = CassMemoryService::new(storage.clone(), intelligence.clone());
    let hydration_svc = CassHydrationService::new(
        storage.clone(),
        hierarchy.clone(),
        codegraph.clone(),
        intelligence.clone(),
    )
    .with_pulse_store(Arc::clone(&redis_tier) as Arc<dyn koad_cass::storage::PulseTier>);
    let pulse_svc =
        CassPulseService::new(Arc::clone(&redis_tier) as Arc<dyn koad_cass::storage::PulseTier>);
    let stream_svc = CassStreamService::new(notion_client.clone(), stream_db);
    let symbol_svc = CassSymbolService::new(codegraph.clone());
    let tool_svc = CassToolRegistryService::new(plugin_registry);

    // Pipelines
    let eow_pipeline = Arc::new(EndOfWatchPipeline::new(
        storage.clone(),
        signal_corps.clone(),
        intelligence.clone(),
    ));
    tokio::spawn(async move {
        eow_pipeline.start_listener().await;
    });

    let grpc_port = std::env::var("CASS_GRPC_PORT")
        .unwrap_or_else(|_| "50052".to_string());
    let addr = format!("0.0.0.0:{}", grpc_port).parse()?;
    info!("CASS: gRPC server listening on {}", addr);

    Server::builder()
        .add_service(MemoryServiceServer::new(memory_svc))
        .add_service(HydrationServiceServer::new(hydration_svc))
        .add_service(PulseServiceServer::new(pulse_svc))
        .add_service(StreamServiceServer::new(stream_svc))
        .add_service(SymbolServiceServer::new(symbol_svc))
        .add_service(ToolRegistryServiceServer::new(tool_svc))
        .serve(addr)
        .await?;

    Ok(())
}
