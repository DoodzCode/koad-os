#[cfg(test)]
use super::{MemoryTier, PulseTier};
use anyhow::Result;
use async_trait::async_trait;
use koad_proto::cass::v1::{EpisodicMemory, FactCard, Pulse};
use std::sync::Arc;
use std::sync::Mutex;
use tokio::sync::Mutex as TokioMutex;

pub struct MockStorage {
    pub facts: Arc<Mutex<Vec<FactCard>>>,
    pub episodes: Arc<Mutex<Vec<EpisodicMemory>>>,
}

pub struct MockPulseStore {
    pulses: Arc<TokioMutex<Vec<Pulse>>>,
}

impl Default for MockPulseStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MockPulseStore {
    pub fn new() -> Self {
        Self {
            pulses: Arc::new(TokioMutex::new(vec![])),
        }
    }

    pub async fn seed(&self, pulse: Pulse) {
        self.pulses.lock().await.push(pulse);
    }

    pub async fn all(&self) -> Vec<Pulse> {
        self.pulses.lock().await.clone()
    }
}

#[async_trait]
impl PulseTier for MockPulseStore {
    async fn add_pulse(&self, pulse: Pulse) -> Result<()> {
        self.pulses.lock().await.push(pulse);
        Ok(())
    }

    async fn get_active_pulses(&self, _role: &str) -> Result<Vec<Pulse>> {
        Ok(self.pulses.lock().await.clone())
    }
}

impl MockStorage {
    pub fn new() -> Self {
        Self {
            facts: Arc::new(Mutex::new(Vec::new())),
            episodes: Arc::new(Mutex::new(Vec::new())),
        }
    }
}

impl Default for MockStorage {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryTier for MockStorage {
    async fn commit_fact(&self, fact: FactCard) -> Result<()> {
        let mut facts = self
            .facts
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        facts.push(fact);
        Ok(())
    }

    async fn query_facts(
        &self,
        _domain: &str,
        _tags: &[String],
        _limit: u32,
    ) -> Result<Vec<FactCard>> {
        let facts = self
            .facts
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        Ok(facts.clone())
    }

    async fn query_agent_facts(
        &self,
        _agent_name: &str,
        _limit: u32,
        _task_id: Option<&str>,
    ) -> Result<Vec<FactCard>> {
        let facts = self
            .facts
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        Ok(facts.clone())
    }

    async fn record_episode(&self, episode: EpisodicMemory) -> Result<()> {
        let mut episodes = self
            .episodes
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        episodes.push(episode);
        Ok(())
    }

    async fn query_recent_episodes(
        &self,
        _agent_name: &str,
        _limit: u32,
        _task_id: Option<&str>,
    ) -> Result<Vec<EpisodicMemory>> {
        let episodes = self
            .episodes
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        Ok(episodes.clone())
    }

    async fn search_semantic(
        &self,
        query: &str,
        _partition: &str,
        limit: u32,
    ) -> Result<Vec<FactCard>> {
        let facts = self
            .facts
            .lock()
            .map_err(|_| anyhow::anyhow!("Mutex poisoned"))?;
        let query_lower = query.to_lowercase();
        let results = facts
            .iter()
            .filter(|f| f.content.to_lowercase().contains(&query_lower))
            .take(limit as usize)
            .cloned()
            .collect();
        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use koad_proto::cass::v1::{FactCard, MemoryMetadata};

    #[tokio::test]
    async fn test_mock_preserves_metadata() -> Result<()> {
        let s = MockStorage::new();
        let f = FactCard {
            id: "m1".into(),
            metadata: Some(MemoryMetadata {
                summary: "keep me".into(),
                ..Default::default()
            }),
            ..Default::default()
        };
        s.commit_fact(f).await?;
        let got = s.query_facts("", &[], 10).await?;
        assert_eq!(got[0].metadata.as_ref().unwrap().summary, "keep me");
        Ok(())
    }
}
