//! L3 — Qdrant-backed semantic storage tier.
//!
//! Facts are stored as dense vector points derived from either content hashing
//! or real InferenceRouter embeddings.
//! Queries use payload filtering by domain for deterministic retrieval.
//! Semantic vector similarity search is performed using SearchPoints.

use crate::storage::MemoryTier;
use anyhow::{Context, Result};
use async_trait::async_trait;
use koad_intelligence::router::InferenceRouter;
use koad_proto::cass::v1::{EpisodicMemory, FactCard};
use qdrant_client::qdrant::{
    value::Kind, Condition, CreateCollectionBuilder, Distance, Filter, PointStruct,
    ScrollPointsBuilder, SearchPointsBuilder, UpsertPointsBuilder, VectorParamsBuilder,
};
use qdrant_client::Qdrant;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::Arc;

const COLLECTION: &str = "fact_cards";
const EPISODE_COLLECTION: &str = "episodic_memories";
const VECTOR_DIM: u64 = 32;

pub struct QdrantTier {
    client: Option<Qdrant>,
    intelligence: Option<Arc<InferenceRouter>>,
    vector_dim: u64,
}

impl QdrantTier {
    /// Create a no-op offline tier for degraded-mode boot (Qdrant unreachable).
    pub fn new_offline() -> Self {
        Self {
            client: None,
            intelligence: None,
            vector_dim: VECTOR_DIM,
        }
    }

    pub async fn new(url: &str, intelligence: Option<Arc<InferenceRouter>>) -> Result<Self> {
        let client = Qdrant::from_url(url)
            .build()
            .context("Failed to build Qdrant client")?;

        // Resolve embedding dimension by sending a dummy string if intelligence is available
        let mut dim = VECTOR_DIM;
        if let Some(ref intel) = intelligence {
            match intel.embed("test").await {
                Ok(vec) => {
                    dim = vec.len() as u64;
                    tracing::info!("Qdrant: Detected embedding model dimension: {}", dim);
                }
                Err(e) => {
                    tracing::warn!(
                        "Qdrant: Failed to query embedding dimension from router ({}), falling back to default {}",
                        e,
                        dim
                    );
                }
            }
        }

        if !client.collection_exists(COLLECTION).await? {
            client
                .create_collection(
                    CreateCollectionBuilder::new(COLLECTION)
                        .vectors_config(VectorParamsBuilder::new(dim, Distance::Cosine)),
                )
                .await
                .context("Failed to create Qdrant collection")?;
        }

        if !client.collection_exists(EPISODE_COLLECTION).await? {
            client
                .create_collection(
                    CreateCollectionBuilder::new(EPISODE_COLLECTION)
                        .vectors_config(VectorParamsBuilder::new(dim, Distance::Cosine)),
                )
                .await
                .context("Failed to create Qdrant episodic_memories collection")?;
        }

        Ok(Self {
            client: Some(client),
            intelligence,
            vector_dim: dim,
        })
    }

    /// Deterministic u64 point ID from fact UUID string.
    fn point_id(fact_id: &str) -> u64 {
        let mut h = DefaultHasher::new();
        fact_id.hash(&mut h);
        h.finish()
    }

    /// Content fingerprint vector derived from content bytes.
    /// Serves as a stable placeholder when no embedding model is available.
    fn content_vector(&self, content: &str) -> Vec<f32> {
        let mut vec = vec![0.0f32; self.vector_dim as usize];
        for (i, &b) in content.as_bytes().iter().enumerate() {
            vec[i % self.vector_dim as usize] += b as f32;
        }
        let mag = (vec.iter().map(|x| x * x).sum::<f32>()).sqrt().max(1e-6);
        vec.iter().map(|x| x / mag).collect()
    }

    /// Generate an embedding vector for the given content.
    /// Falls back to deterministic content hash fingerprint if client is offline or error occurs.
    async fn get_vector(&self, text: &str) -> Vec<f32> {
        if let Some(ref intel) = self.intelligence {
            match intel.embed(text).await {
                Ok(vec) => {
                    if vec.len() as u64 == self.vector_dim {
                        return vec;
                    }
                    tracing::warn!(
                        "QdrantTier: embedding dimension mismatch (expected {}, got {}). Using fallback.",
                        self.vector_dim,
                        vec.len()
                    );
                }
                Err(e) => {
                    tracing::warn!("QdrantTier: failed to get embedding ({}). Using fallback.", e);
                }
            }
        }
        self.content_vector(text)
    }

    fn make_payload(fact: &FactCard) -> HashMap<String, qdrant_client::qdrant::Value> {
        use qdrant_client::qdrant::Value;
        let mut p = HashMap::new();
        p.insert(
            "id".into(),
            Value {
                kind: Some(Kind::StringValue(fact.id.clone())),
            },
        );
        p.insert(
            "domain".into(),
            Value {
                kind: Some(Kind::StringValue(fact.domain.clone())),
            },
        );
        p.insert(
            "content".into(),
            Value {
                kind: Some(Kind::StringValue(fact.content.clone())),
            },
        );
        p.insert(
            "source_agent".into(),
            Value {
                kind: Some(Kind::StringValue(fact.source_agent.clone())),
            },
        );
        p.insert(
            "session_id".into(),
            Value {
                kind: Some(Kind::StringValue(fact.session_id.clone())),
            },
        );
        p.insert(
            "confidence".into(),
            Value {
                kind: Some(Kind::DoubleValue(fact.confidence as f64)),
            },
        );
        p.insert(
            "tags".into(),
            Value {
                kind: Some(Kind::StringValue(fact.tags.join(","))),
            },
        );
        p
    }

    fn payload_to_fact(
        payload: &HashMap<String, qdrant_client::qdrant::Value>,
    ) -> Option<FactCard> {
        let get_str = |key: &str| -> Option<String> {
            match payload.get(key)?.kind.as_ref()? {
                Kind::StringValue(s) => Some(s.clone()),
                _ => None,
            }
        };
        let get_f64 = |key: &str| -> Option<f64> {
            match payload.get(key)?.kind.as_ref()? {
                Kind::DoubleValue(d) => Some(*d),
                _ => None,
            }
        };

        Some(FactCard {
            id: get_str("id")?,
            domain: get_str("domain")?,
            content: get_str("content")?,
            source_agent: get_str("source_agent")?,
            session_id: get_str("session_id")?,
            confidence: get_f64("confidence")? as f32,
            tags: get_str("tags")?.split(',').map(|s| s.to_string()).collect(),
            created_at: None,
        })
    }

    fn make_episode_payload(episode: &EpisodicMemory) -> HashMap<String, qdrant_client::qdrant::Value> {
        use qdrant_client::qdrant::Value;
        let mut p = HashMap::new();
        p.insert(
            "session_id".into(),
            Value {
                kind: Some(Kind::StringValue(episode.session_id.clone())),
            },
        );
        p.insert(
            "project_path".into(),
            Value {
                kind: Some(Kind::StringValue(episode.project_path.clone())),
            },
        );
        p.insert(
            "summary".into(),
            Value {
                kind: Some(Kind::StringValue(episode.summary.clone())),
            },
        );
        p.insert(
            "turn_count".into(),
            Value {
                kind: Some(Kind::IntegerValue(episode.turn_count as i64)),
            },
        );
        p.insert(
            "task_ids".into(),
            Value {
                kind: Some(Kind::StringValue(episode.task_ids.join(","))),
            },
        );
        let seconds = episode.timestamp.as_ref().map(|t| t.seconds).unwrap_or(0);
        p.insert(
            "timestamp".into(),
            Value {
                kind: Some(Kind::IntegerValue(seconds)),
            },
        );
        p
    }

    fn payload_to_episode(
        payload: &HashMap<String, qdrant_client::qdrant::Value>,
    ) -> Option<EpisodicMemory> {
        let get_str = |key: &str| -> Option<String> {
            match payload.get(key)?.kind.as_ref()? {
                Kind::StringValue(s) => Some(s.clone()),
                _ => None,
            }
        };
        let get_i64 = |key: &str| -> Option<i64> {
            match payload.get(key)?.kind.as_ref()? {
                Kind::IntegerValue(i) => Some(*i),
                _ => None,
            }
        };

        Some(EpisodicMemory {
            session_id: get_str("session_id")?,
            project_path: get_str("project_path")?,
            summary: get_str("summary")?,
            turn_count: get_i64("turn_count")? as u32,
            task_ids: get_str("task_ids")?
                .split(',')
                .filter(|s| !s.is_empty())
                .map(|s| s.to_string())
                .collect(),
            timestamp: Some(prost_types::Timestamp {
                seconds: get_i64("timestamp").unwrap_or(0),
                nanos: 0,
            }),
        })
    }

    pub async fn commit_facts(&self, facts: Vec<FactCard>) -> Result<()> {
        let Some(client) = &self.client else {
            return Ok(());
        };
        if facts.is_empty() {
            return Ok(());
        }

        let mut points = Vec::with_capacity(facts.len());
        for fact in facts {
            let vector = self.get_vector(&fact.content).await;
            let payload = Self::make_payload(&fact);
            points.push(PointStruct::new(Self::point_id(&fact.id), vector, payload));
        }

        client
            .upsert_points(UpsertPointsBuilder::new(COLLECTION, points))
            .await
            .context("QdrantTier: batch upsert failed")?;
        Ok(())
    }
}

#[async_trait]
impl MemoryTier for QdrantTier {
    async fn commit_fact(&self, fact: FactCard) -> Result<()> {
        let Some(client) = &self.client else {
            return Ok(());
        };
        let vector = self.get_vector(&fact.content).await;
        let payload = Self::make_payload(&fact);
        let point = PointStruct::new(Self::point_id(&fact.id), vector, payload);

        client
            .upsert_points(UpsertPointsBuilder::new(COLLECTION, vec![point]))
            .await
            .context("QdrantTier: upsert failed")?;
        Ok(())
    }

    async fn query_facts(
        &self,
        domain: &str,
        _tags: &[String],
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        let Some(client) = &self.client else {
            return Ok(vec![]);
        };
        let filter = Filter::must([Condition::matches("domain", domain.to_string())]);

        let result = client
            .scroll(
                ScrollPointsBuilder::new(COLLECTION)
                    .filter(filter)
                    .limit(limit)
                    .with_payload(true),
            )
            .await
            .context("QdrantTier: scroll failed")?;

        let facts = result
            .result
            .iter()
            .filter_map(|p| Self::payload_to_fact(&p.payload))
            .collect();

        Ok(facts)
    }

    async fn query_agent_facts(
        &self,
        _agent_name: &str,
        _limit: u32,
        _task_id: Option<&str>,
    ) -> Result<Vec<FactCard>> {
        // Qdrant tier defers agent-scoped queries to SQLite (L2).
        Ok(vec![])
    }

    async fn record_episode(&self, episode: EpisodicMemory) -> Result<()> {
        let Some(client) = &self.client else {
            return Ok(());
        };
        let vector = self.get_vector(&episode.summary).await;
        let payload = Self::make_episode_payload(&episode);
        let point_id = Self::point_id(&episode.session_id);
        let point = PointStruct::new(point_id, vector, payload);

        client
            .upsert_points(UpsertPointsBuilder::new(EPISODE_COLLECTION, vec![point]))
            .await
            .context("QdrantTier: record_episode failed")?;
        Ok(())
    }

    async fn query_recent_episodes(
        &self,
        agent_name: &str,
        limit: u32,
        _task_id: Option<&str>,
    ) -> Result<Vec<EpisodicMemory>> {
        let Some(client) = &self.client else {
            return Ok(vec![]);
        };

        let result = client
            .scroll(
                ScrollPointsBuilder::new(EPISODE_COLLECTION)
                    .limit(limit)
                    .with_payload(true),
            )
            .await
            .context("QdrantTier: query_recent_episodes scroll failed")?;

        let episodes = result
            .result
            .iter()
            .filter_map(|p| Self::payload_to_episode(&p.payload))
            .filter(|ep| ep.session_id.contains(agent_name))
            .collect();

        Ok(episodes)
    }

    async fn search_semantic(
        &self,
        query: &str,
        partition: &str,
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        let Some(client) = &self.client else {
            return Ok(vec![]);
        };

        let vector = self.get_vector(query).await;

        // Query 1: Search facts
        let filter_facts = Filter::must([Condition::matches(
            "source_agent",
            partition.to_string(),
        )]);
        let result_facts = match client
            .search_points(
                SearchPointsBuilder::new(
                    COLLECTION,
                    vector.clone(),
                    limit as u64,
                )
                .filter(filter_facts)
                .with_payload(true),
            )
            .await
        {
            Ok(res) => res.result,
            Err(e) => {
                tracing::warn!("Qdrant: search_points on facts failed: {}", e);
                vec![]
            }
        };

        // Query 2: Search episodic memories
        let result_episodes = match client
            .search_points(
                SearchPointsBuilder::new(
                    EPISODE_COLLECTION,
                    vector,
                    limit as u64,
                )
                .with_payload(true),
            )
            .await
        {
            Ok(res) => {
                // Filter locally by agent partition since EpisodicMemory does not store partition natively
                res.result
                    .into_iter()
                    .filter(|p| {
                        if let Some(Kind::StringValue(sid)) = p.payload.get("session_id").and_then(|v| v.kind.as_ref()) {
                            sid.contains(partition)
                        } else {
                            false
                        }
                    })
                    .collect()
            }
            Err(e) => {
                tracing::warn!("Qdrant: search_points on episodes failed: {}", e);
                vec![]
            }
        };

        // Map and merge
        let mut scored_facts: Vec<(f32, FactCard)> = Vec::new();

        for p in result_facts {
            if let Some(fact) = Self::payload_to_fact(&p.payload) {
                scored_facts.push((p.score, fact));
            }
        }

        for p in result_episodes {
            if let Some(ep) = Self::payload_to_episode(&p.payload) {
                let fact = FactCard {
                    id: ep.session_id.clone(),
                    source_agent: partition.to_string(),
                    session_id: ep.session_id.clone(),
                    domain: "session".to_string(),
                    content: format!("## Session Summary: {}\n\n{}", ep.session_id, ep.summary),
                    confidence: 1.0,
                    tags: vec!["session_summary".to_string()],
                    created_at: ep.timestamp,
                };
                scored_facts.push((p.score, fact));
            }
        }

        // Sort descending by score
        scored_facts.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

        // Truncate to limit
        let merged = scored_facts
            .into_iter()
            .take(limit as usize)
            .map(|(_, fact)| fact)
            .collect();

        Ok(merged)
    }
}
