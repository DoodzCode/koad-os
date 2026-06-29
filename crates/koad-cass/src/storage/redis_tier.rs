//! L1 — Redis-backed hot cache tier.
//!
//! Facts are stored as JSON strings with a 1-hour TTL.
//! Domain membership is tracked via Redis sets for O(1) lookup.

use crate::storage::{MemoryTier, PulseTier};
use anyhow::Result;
use async_trait::async_trait;
use fred::clients::RedisPool;
use fred::interfaces::{KeysInterface, SetsInterface};
use koad_proto::cass::v1::{EpisodicMemory, FactCard, Pulse};
use tracing::warn;

pub struct RedisTier {
    pub(crate) pool: RedisPool,
}

impl RedisTier {
    pub fn new(pool: RedisPool) -> Self {
        Self { pool }
    }

    fn fact_key(id: &str) -> String {
        format!("cass:fact:{}", id)
    }

    fn domain_set_key(domain: &str) -> String {
        format!("cass:domain:{}", domain)
    }

    fn fact_to_json(fact: &FactCard) -> String {
        serde_json::json!({
            "id": fact.id,
            "domain": fact.domain,
            "content": fact.content,
            "source_agent": fact.source_agent,
            "session_id": fact.session_id,
            "confidence": fact.confidence,
            "tags": fact.tags.join(","),
        })
        .to_string()
    }

    fn json_to_fact(json: &str) -> Option<FactCard> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        Some(FactCard {
            id: v["id"].as_str()?.to_string(),
            domain: v["domain"].as_str()?.to_string(),
            content: v["content"].as_str()?.to_string(),
            source_agent: v["source_agent"].as_str()?.to_string(),
            session_id: v["session_id"].as_str()?.to_string(),
            confidence: v["confidence"].as_f64()? as f32,
            tags: v["tags"]
                .as_str()?
                .split(',')
                .map(|s| s.to_string())
                .collect(),
            created_at: None,
            metadata: None,
        })
    }
}

const TTL: i64 = 3600;

#[async_trait]
impl MemoryTier for RedisTier {
    async fn commit_fact(&self, fact: FactCard) -> Result<()> {
        let client = self.pool.next();
        let json = Self::fact_to_json(&fact);
        let fact_key = Self::fact_key(&fact.id);
        let domain_key = Self::domain_set_key(&fact.domain);

        client
            .set::<(), _, _>(
                &fact_key,
                &json,
                Some(fred::types::Expiration::EX(TTL)),
                None,
                false,
            )
            .await?;
        client
            .sadd::<(), _, _>(&domain_key, fact.id.as_str())
            .await?;
        client.expire::<(), _>(&domain_key, TTL).await?;
        Ok(())
    }

    async fn query_facts(
        &self,
        domain: &str,
        _tags: &[String],
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        let client = self.pool.next();
        let domain_key = Self::domain_set_key(domain);

        let ids: Vec<String> = client.smembers(&domain_key).await.unwrap_or_default();
        if ids.is_empty() {
            return Ok(vec![]);
        }

        let mut facts = Vec::new();
        for id in ids.iter().take(limit as usize) {
            let json: Option<String> = client.get(Self::fact_key(id)).await.unwrap_or(None);
            if let Some(json) = json {
                match Self::json_to_fact(&json) {
                    Some(fact) => facts.push(fact),
                    None => warn!(id = %id, "RedisTier: Failed to deserialize fact"),
                }
            }
        }

        facts.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(facts)
    }

    async fn query_agent_facts(
        &self,
        _agent_name: &str,
        _limit: u32,
        _task_id: Option<&str>,
    ) -> Result<Vec<FactCard>> {
        // Redis tier does not index by agent; L2 (SQLite) is authoritative for agent queries.
        Ok(vec![])
    }

    async fn record_episode(&self, _episode: EpisodicMemory) -> Result<()> {
        // Redis tier does not persist episodes.
        Ok(())
    }

    async fn query_recent_episodes(
        &self,
        _agent_name: &str,
        _limit: u32,
        _task_id: Option<&str>,
    ) -> Result<Vec<EpisodicMemory>> {
        Ok(vec![])
    }

    async fn search_semantic(
        &self,
        _query: &str,
        _partition: &str,
        _limit: u32,
    ) -> Result<Vec<FactCard>> {
        // Redis has no full-text search capability.
        Ok(vec![])
    }
}

impl RedisTier {
    pub(crate) fn pulse_key(id: &str) -> String {
        format!("koad:pulse:{}", id)
    }

    fn role_set_key(role: &str) -> String {
        format!("koad:pulse:role:{}", role)
    }

    fn pulse_to_json(pulse: &Pulse) -> String {
        serde_json::json!({
            "id": pulse.id,
            "author": pulse.author,
            "role": pulse.role,
            "message": pulse.message,
            "ttl_seconds": pulse.ttl_seconds,
        })
        .to_string()
    }

    fn json_to_pulse(json: &str) -> Option<Pulse> {
        let v: serde_json::Value = serde_json::from_str(json).ok()?;
        Some(Pulse {
            id: v["id"].as_str()?.to_string(),
            author: v["author"].as_str()?.to_string(),
            role: v["role"].as_str()?.to_string(),
            message: v["message"].as_str()?.to_string(),
            ttl_seconds: v["ttl_seconds"].as_u64()? as u32,
            created_at: None,
        })
    }
}

#[async_trait]
impl PulseTier for RedisTier {
    async fn add_pulse(&self, pulse: Pulse) -> Result<()> {
        let client = self.pool.next();
        let ttl = if pulse.ttl_seconds == 0 {
            TTL
        } else {
            pulse.ttl_seconds as i64
        };
        let json = Self::pulse_to_json(&pulse);
        let key = Self::pulse_key(&pulse.id);

        client
            .set::<(), _, _>(
                &key,
                &json,
                Some(fred::types::Expiration::EX(ttl)),
                None,
                false,
            )
            .await?;

        // Index under role set
        let role_key = Self::role_set_key(&pulse.role);
        client
            .sadd::<(), _, _>(&role_key, pulse.id.as_str())
            .await?;
        client.expire::<(), _>(&role_key, ttl).await?;

        Ok(())
    }

    async fn get_active_pulses(&self, role: &str) -> Result<Vec<Pulse>> {
        let client = self.pool.next();
        let mut pulses = Vec::new();

        // Collect IDs from the requested role
        let mut ids: Vec<String> = client
            .smembers(Self::role_set_key(role))
            .await
            .unwrap_or_default();

        // Also include global pulses if querying a specific role
        if role != "global" {
            let global_ids: Vec<String> = client
                .smembers(Self::role_set_key("global"))
                .await
                .unwrap_or_default();
            ids.extend(global_ids);
        }
        ids.dedup();

        for id in ids {
            let json: Option<String> = client.get(Self::pulse_key(&id)).await.unwrap_or(None);
            if let Some(json) = json {
                if let Some(pulse) = Self::json_to_pulse(&json) {
                    pulses.push(pulse);
                }
            }
        }

        Ok(pulses)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::PulseTier;
    use fred::interfaces::KeysInterface;

    #[tokio::test]
    #[ignore = "requires: Redis at ~/.koad-os/run/koad.sock"]
    async fn test_pulse_round_trip() -> anyhow::Result<()> {
        let koad_home = std::env::var("KOADOS_HOME")
            .unwrap_or_else(|_| format!("{}/.koad-os", std::env::var("HOME").unwrap_or_default()));
        let redis = koad_core::utils::redis::RedisClient::new(&koad_home, false).await?;
        let tier = RedisTier::new(redis.pool.clone());

        let pulse = Pulse {
            id: "test-pulse-rtt-001".to_string(),
            author: "test".to_string(),
            role: "global".to_string(),
            message: "Round trip test".to_string(),
            ttl_seconds: 60,
            created_at: None,
        };

        tier.add_pulse(pulse).await?;
        let results = tier.get_active_pulses("global").await?;
        assert!(results.iter().any(|p| p.id == "test-pulse-rtt-001"));

        // Cleanup — delete the pulse key; TTL will expire the role set entry
        let client = tier.pool.next();
        client
            .del::<(), _>(RedisTier::pulse_key("test-pulse-rtt-001"))
            .await?;

        Ok(())
    }
}
