//! L2 — SQLite-backed durable storage tier.

use crate::storage::MemoryTier;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use koad_proto::cass::v1::{EpisodicMemory, FactCard};
use rusqlite::{params, Connection};
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct SqliteTier {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteTier {
    pub fn new(db_path: &str) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS fact_cards (
                id TEXT PRIMARY KEY,
                source_agent TEXT NOT NULL,
                session_id TEXT NOT NULL,
                domain TEXT NOT NULL,
                content TEXT NOT NULL,
                confidence REAL NOT NULL,
                tags TEXT NOT NULL,
                created_at TEXT NOT NULL,
                task_ids TEXT
            )",
            [],
        )?;
        conn.execute(
            "CREATE TABLE IF NOT EXISTS episodic_memories (
                session_id TEXT PRIMARY KEY,
                project_path TEXT NOT NULL,
                summary TEXT NOT NULL,
                turn_count INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                task_ids TEXT NOT NULL
            )",
            [],
        )?;
        // Idempotent metadata migrations (ignore "duplicate column" on existing DBs).
        let _ = conn.execute("ALTER TABLE fact_cards ADD COLUMN metadata_json TEXT", []);
        let _ = conn.execute("ALTER TABLE episodic_memories ADD COLUMN metadata_json TEXT", []);
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

fn metadata_to_json(md: &Option<koad_proto::cass::v1::MemoryMetadata>) -> Option<String> {
    md.as_ref().and_then(|m| serde_json::to_string(m).ok())
}

fn metadata_from_json(raw: Option<String>) -> Option<koad_proto::cass::v1::MemoryMetadata> {
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

#[async_trait]
impl MemoryTier for SqliteTier {
    async fn commit_fact(&self, fact: FactCard) -> Result<()> {
        let conn = self.conn.lock().await;
        let tags = fact.tags.join(",");
        let created_at = Utc::now().to_rfc3339();
        let metadata_json = metadata_to_json(&fact.metadata);
        conn.execute(
            "INSERT OR REPLACE INTO fact_cards
             (id, source_agent, session_id, domain, content, confidence, tags, created_at, metadata_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                fact.id,
                fact.source_agent,
                fact.session_id,
                fact.domain,
                fact.content,
                fact.confidence,
                tags,
                created_at,
                metadata_json
            ],
        )?;
        Ok(())
    }

    async fn query_facts(
        &self,
        domain: &str,
        _tags: &[String],
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        let conn = self.conn.lock().await;
        let domain_prefix = format!("{}:%", domain);
        let mut stmt = conn.prepare(
            "SELECT id, source_agent, session_id, domain, content, confidence, tags, metadata_json
             FROM fact_cards
             WHERE domain = ?1 OR domain LIKE ?2
             ORDER BY confidence DESC LIMIT ?3",
        )?;
        let rows = stmt.query_map(params![domain, domain_prefix, limit], |row| {
            Ok(FactCard {
                id: row.get(0)?,
                source_agent: row.get(1)?,
                session_id: row.get(2)?,
                domain: row.get(3)?,
                content: row.get(4)?,
                confidence: row.get(5)?,
                tags: row
                    .get::<_, String>(6)?
                    .split(',')
                    .map(|s| s.to_string())
                    .collect(),
                created_at: None,
                metadata: metadata_from_json(row.get::<_, Option<String>>(7)?),
            })
        })?;
        let mut facts = Vec::new();
        for row in rows {
            facts.push(row?);
        }
        Ok(facts)
    }

    async fn query_agent_facts(
        &self,
        agent_name: &str,
        limit: u32,
        task_id: Option<&str>,
    ) -> Result<Vec<FactCard>> {
        let conn = self.conn.lock().await;
        let mut facts = Vec::new();
        if let Some(tid) = task_id.filter(|t| !t.is_empty()) {
            let mut stmt = conn.prepare(
                "SELECT id, source_agent, session_id, domain, content, confidence, tags, metadata_json
                 FROM fact_cards WHERE source_agent = ?1 AND task_ids LIKE '%' || ?2 || '%'
                 ORDER BY confidence DESC LIMIT ?3",
            )?;
            let rows = stmt.query_map(params![agent_name, tid, limit], |row| {
                Ok(FactCard {
                    id: row.get(0)?,
                    source_agent: row.get(1)?,
                    session_id: row.get(2)?,
                    domain: row.get(3)?,
                    content: row.get(4)?,
                    confidence: row.get(5)?,
                    tags: row
                        .get::<_, String>(6)?
                        .split(',')
                        .map(|s| s.to_string())
                        .collect(),
                    created_at: None,
                    metadata: metadata_from_json(row.get::<_, Option<String>>(7)?),
                })
            })?;
            for row in rows {
                facts.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT id, source_agent, session_id, domain, content, confidence, tags, metadata_json
                 FROM fact_cards WHERE source_agent = ?1 ORDER BY confidence DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![agent_name, limit], |row| {
                Ok(FactCard {
                    id: row.get(0)?,
                    source_agent: row.get(1)?,
                    session_id: row.get(2)?,
                    domain: row.get(3)?,
                    content: row.get(4)?,
                    confidence: row.get(5)?,
                    tags: row
                        .get::<_, String>(6)?
                        .split(',')
                        .map(|s| s.to_string())
                        .collect(),
                    created_at: None,
                    metadata: metadata_from_json(row.get::<_, Option<String>>(7)?),
                })
            })?;
            for row in rows {
                facts.push(row?);
            }
        }
        Ok(facts)
    }

    async fn record_episode(&self, episode: EpisodicMemory) -> Result<()> {
        let conn = self.conn.lock().await;
        let task_ids = episode.task_ids.join(",");
        let timestamp = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR REPLACE INTO episodic_memories
             (session_id, project_path, summary, turn_count, timestamp, task_ids)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                episode.session_id,
                episode.project_path,
                episode.summary,
                episode.turn_count,
                timestamp,
                task_ids
            ],
        )?;
        Ok(())
    }

    async fn search_semantic(
        &self,
        query: &str,
        partition: &str,
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        let conn = self.conn.lock().await;
        let pattern = format!("%{}%", query);
        let domain_prefix = format!("{}:%", partition);
        let mut stmt = conn.prepare(
            "SELECT id, source_agent, session_id, domain, content, confidence, tags, metadata_json
             FROM fact_cards
             WHERE content LIKE ?1 AND (domain = ?2 OR domain LIKE ?3)
             ORDER BY confidence DESC LIMIT ?4",
        )?;
        let rows = stmt.query_map(params![pattern, partition, domain_prefix, limit], |row| {
            Ok(FactCard {
                id: row.get(0)?,
                source_agent: row.get(1)?,
                session_id: row.get(2)?,
                domain: row.get(3)?,
                content: row.get(4)?,
                confidence: row.get(5)?,
                tags: row
                    .get::<_, String>(6)?
                    .split(',')
                    .map(|s| s.to_string())
                    .collect(),
                created_at: None,
                metadata: metadata_from_json(row.get::<_, Option<String>>(7)?),
            })
        })?;
        let mut facts = Vec::new();
        for row in rows {
            facts.push(row?);
        }
        Ok(facts)
    }

    async fn query_recent_episodes(
        &self,
        _agent_name: &str,
        limit: u32,
        task_id: Option<&str>,
    ) -> Result<Vec<EpisodicMemory>> {
        let conn = self.conn.lock().await;
        let mut episodes = Vec::new();
        if let Some(tid) = task_id.filter(|t| !t.is_empty()) {
            let mut stmt = conn.prepare(
                "SELECT session_id, project_path, summary, turn_count, timestamp, task_ids, metadata_json
                 FROM episodic_memories WHERE task_ids LIKE '%' || ?1 || '%'
                 ORDER BY timestamp DESC LIMIT ?2",
            )?;
            let rows = stmt.query_map(params![tid, limit], |row| {
                Ok(EpisodicMemory {
                    session_id: row.get(0)?,
                    project_path: row.get(1)?,
                    summary: row.get(2)?,
                    turn_count: row.get(3)?,
                    timestamp: None,
                    task_ids: row
                        .get::<_, String>(5)?
                        .split(',')
                        .map(|s| s.to_string())
                        .collect(),
                    metadata: metadata_from_json(row.get::<_, Option<String>>(6)?),
                })
            })?;
            for row in rows {
                episodes.push(row?);
            }
        } else {
            let mut stmt = conn.prepare(
                "SELECT session_id, project_path, summary, turn_count, timestamp, task_ids, metadata_json
                 FROM episodic_memories ORDER BY timestamp DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit], |row| {
                Ok(EpisodicMemory {
                    session_id: row.get(0)?,
                    project_path: row.get(1)?,
                    summary: row.get(2)?,
                    turn_count: row.get(3)?,
                    timestamp: None,
                    task_ids: row
                        .get::<_, String>(5)?
                        .split(',')
                        .map(|s| s.to_string())
                        .collect(),
                    metadata: metadata_from_json(row.get::<_, Option<String>>(6)?),
                })
            })?;
            for row in rows {
                episodes.push(row?);
            }
        }
        Ok(episodes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koad_proto::cass::v1::{EpisodicMemory, FactCard};

    fn fact(id: &str, partition: &str, topic: &str, content: &str) -> FactCard {
        FactCard {
            id: id.to_string(),
            source_agent: "hermes".to_string(),
            session_id: format!("{partition}-session"),
            domain: format!("{partition}:{topic}"),
            content: content.to_string(),
            confidence: 1.0,
            tags: vec![partition.to_string(), topic.to_string()],
            created_at: None,
            metadata: None,
        }
    }

    #[tokio::test]
    async fn test_query_facts_by_partition_returns_topic_domains() -> Result<()> {
        let storage = SqliteTier::new(":memory:")?;
        let partition = "hermes_jupiter_ideans";

        storage
            .commit_fact(fact(
                "roundtrip-001",
                partition,
                "general",
                "Hermes CASS round trip should recall partition-topic facts",
            ))
            .await?;
        storage
            .commit_fact(fact(
                "other-001",
                "rook_jupiter_ideans",
                "general",
                "Rook fact should not leak into Hermes partition recall",
            ))
            .await?;

        let results = storage.query_facts(partition, &[], 10).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "roundtrip-001");
        assert_eq!(results[0].domain, "hermes_jupiter_ideans:general");

        Ok(())
    }

    #[tokio::test]
    async fn test_search_semantic_filters_by_partition_domain_not_source_agent() -> Result<()> {
        let storage = SqliteTier::new(":memory:")?;
        let partition = "hermes_jupiter_ideans";

        storage
            .commit_fact(fact(
                "semantic-001",
                partition,
                "recall",
                "CASS semantic search should find this Hermes recall memory",
            ))
            .await?;
        storage
            .commit_fact(fact(
                "semantic-other-001",
                "rook_jupiter_ideans",
                "recall",
                "CASS semantic search should not return this other recall memory",
            ))
            .await?;

        let results = storage.search_semantic("Hermes recall", partition, 10).await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "semantic-001");

        Ok(())
    }

    #[tokio::test]
    async fn test_sqlite_tier_filters_by_task() -> Result<()> {
        let storage = SqliteTier::new(":memory:")?;

        storage
            .record_episode(EpisodicMemory {
                session_id: "S1".to_string(),
                project_path: "/root".to_string(),
                summary: "Task A work".to_string(),
                turn_count: 5,
                timestamp: None,
                task_ids: vec!["task-a".to_string()],
                metadata: None,
            })
            .await?;

        storage
            .record_episode(EpisodicMemory {
                session_id: "S2".to_string(),
                project_path: "/root".to_string(),
                summary: "Task B work".to_string(),
                turn_count: 5,
                timestamp: None,
                task_ids: vec!["task-b".to_string()],
                metadata: None,
            })
            .await?;

        let results = storage
            .query_recent_episodes("tyr", 10, Some("task-a"))
            .await?;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].session_id, "S1");

        let all = storage.query_recent_episodes("tyr", 10, None).await?;
        assert_eq!(all.len(), 2);

        Ok(())
    }

    #[tokio::test]
    async fn test_fact_metadata_round_trips() -> Result<()> {
        use koad_proto::cass::v1::{MemoryMetadata, PromptBudgetHints};
        let storage = SqliteTier::new(":memory:")?;
        let mut f = fact("meta-001", "hermes_jupiter_ideans", "general", "card with metadata");
        f.metadata = Some(MemoryMetadata {
            prompt_budget: Some(PromptBudgetHints {
                priority: "high".into(),
                injection_mode: "summary".into(),
                cache_stable: true,
                ..Default::default()
            }),
            summary: "short form".into(),
            ..Default::default()
        });
        storage.commit_fact(f).await?;
        let got = storage.query_facts("hermes_jupiter_ideans", &[], 10).await?;
        assert_eq!(got.len(), 1);
        let md = got[0].metadata.as_ref().expect("metadata present");
        assert_eq!(md.summary, "short form");
        assert_eq!(md.prompt_budget.as_ref().unwrap().priority, "high");
        Ok(())
    }

    #[tokio::test]
    async fn test_legacy_rows_without_metadata_query_ok() -> Result<()> {
        let storage = SqliteTier::new(":memory:")?;
        storage.commit_fact(fact("legacy-001", "hermes_jupiter_ideans", "general", "no metadata")).await?;
        let got = storage.query_facts("hermes_jupiter_ideans", &[], 10).await?;
        assert_eq!(got.len(), 1);
        assert!(got[0].metadata.is_none());
        Ok(())
    }
}
