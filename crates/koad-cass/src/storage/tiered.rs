//! Tiered Memory Orchestrator — L1 (Redis) → L2 (SQLite) → L3 (Qdrant).
//!
//! Write path: L1 + L2 synchronously, L3 fire-and-forget.
//! Read path (query_facts): L1 first; fall through to L2 on cache miss.
//! Read path (episodes, agent facts): L2 only (authoritative durable store).

use crate::storage::{MemoryTier, QdrantTier, RedisTier, SqliteTier};
use anyhow::Result;
use async_trait::async_trait;
use koad_proto::cass::v1::{EpisodicMemory, FactCard};
use std::sync::Arc;
use tracing::{error, warn};

pub struct TieredStorage {
    l1: Arc<RedisTier>,
    l2: Arc<SqliteTier>,
    l3: Arc<QdrantTier>,
}

impl TieredStorage {
    pub fn new(l1: Arc<RedisTier>, l2: Arc<SqliteTier>, l3: Arc<QdrantTier>) -> Self {
        Self { l1, l2, l3 }
    }
}

#[async_trait]
impl MemoryTier for TieredStorage {
    async fn commit_fact(&self, fact: FactCard) -> Result<()> {
        // L1: hot cache write (non-fatal on failure)
        if let Err(e) = self.l1.commit_fact(fact.clone()).await {
            warn!(error = %e, "TieredStorage: L1 write failed, continuing");
        }

        // L2: durable write (authoritative)
        self.l2.commit_fact(fact.clone()).await?;

        // L3: semantic index — fire-and-forget
        let l3 = self.l3.clone();
        tokio::spawn(async move {
            if let Err(e) = l3.commit_fact(fact).await {
                error!(error = %e, "TieredStorage: L3 write failed");
            }
        });

        Ok(())
    }

    async fn query_facts(
        &self,
        domain: &str,
        tags: &[String],
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        // Try L1 (hot cache)
        match self.l1.query_facts(domain, tags, limit).await {
            Ok(facts) if !facts.is_empty() => return Ok(facts),
            Err(e) => warn!(error = %e, "TieredStorage: L1 query failed, falling through"),
            Ok(_) => {} // cache miss — fall through
        }

        // Fall through to L2 (SQLite)
        self.l2.query_facts(domain, tags, limit).await
    }

    async fn query_agent_facts(
        &self,
        agent_name: &str,
        limit: u32,
        task_id: Option<&str>,
    ) -> Result<Vec<FactCard>> {
        self.l2.query_agent_facts(agent_name, limit, task_id).await
    }

    async fn record_episode(&self, episode: EpisodicMemory) -> Result<()> {
        self.l2.record_episode(episode.clone()).await?;

        let l3 = self.l3.clone();
        tokio::spawn(async move {
            if let Err(e) = l3.record_episode(episode).await {
                tracing::error!(error = %e, "TieredStorage: L3 record_episode failed");
            }
        });

        Ok(())
    }

    async fn query_recent_episodes(
        &self,
        agent_name: &str,
        limit: u32,
        task_id: Option<&str>,
    ) -> Result<Vec<EpisodicMemory>> {
        self.l2
            .query_recent_episodes(agent_name, limit, task_id)
            .await
    }

    async fn search_semantic(
        &self,
        query: &str,
        partition: &str,
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        // Try L3 (Qdrant vector search) first
        match self.l3.search_semantic(query, partition, limit).await {
            Ok(facts) if !facts.is_empty() => return Ok(facts),
            Err(e) => tracing::warn!(error = %e, "TieredStorage: L3 semantic search failed, falling through to L2 text match"),
            Ok(_) => {}
        }
        self.l2.search_semantic(query, partition, limit).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{QdrantTier, RedisTier, SqliteTier};
    use koad_proto::cass::v1::FactCard;
    use std::sync::Arc;

    async fn make_tiered() -> anyhow::Result<TieredStorage> {
        let sqlite = Arc::new(SqliteTier::new(":memory:")?);
        // Use live local Redis and Qdrant for integration validation
        let redis_client = koad_core::utils::redis::RedisClient::new(
            &std::env::var("KOADOS_HOME").unwrap_or_else(|_| {
                format!(
                    "{}/.koad-os",
                    std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string())
                )
            }),
            false,
        )
        .await?;
        let l1 = Arc::new(RedisTier::new(redis_client.pool.clone()));
        let l3 = Arc::new(match QdrantTier::new("http://127.0.0.1:6334", None).await {
            Ok(q) => q,
            Err(_) => QdrantTier::new_offline(),
        });
        Ok(TieredStorage::new(l1, sqlite, l3))
    }

    #[tokio::test]
    #[ignore = "requires live services (redis, qdrant)"]
    async fn test_tiered_write_and_read() -> anyhow::Result<()> {
        let storage = make_tiered().await?;

        let fact = FactCard {
            id: "tiered-test-001".to_string(),
            domain: "test-tiered".to_string(),
            content: "Tiered memory write-through verified".to_string(),
            source_agent: "clyde".to_string(),
            session_id: "S-TEST".to_string(),
            confidence: 0.9,
            tags: vec!["test".to_string()],
            created_at: None,
            metadata: None,
        };

        // Write through all tiers
        storage.commit_fact(fact.clone()).await?;
        // Small delay for L3 fire-and-forget
        tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

        // Read back (should hit L1 Redis cache)
        let results = storage.query_facts("test-tiered", &[], 5).await?;
        assert!(!results.is_empty(), "Expected fact from L1/L2");
        assert_eq!(results[0].content, fact.content);

        Ok(())
    }
}
