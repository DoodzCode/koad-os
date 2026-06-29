use crate::storage::MemoryTier;
use chrono::Utc;
use koad_core::signal::SignalCorps;
use koad_intelligence::router::InferenceRouter;
use koad_proto::cass::v1::EpisodicMemory;
use std::sync::Arc;
use tracing::{error, info, warn};

pub struct EndOfWatchPipeline {
    storage: Arc<dyn MemoryTier>,
    signal_corps: Arc<SignalCorps>,
    intelligence: Arc<InferenceRouter>,
}

impl EndOfWatchPipeline {
    pub fn new(
        storage: Arc<dyn MemoryTier>,
        signal_corps: Arc<SignalCorps>,
        intelligence: Arc<InferenceRouter>,
    ) -> Self {
        Self {
            storage,
            signal_corps,
            intelligence,
        }
    }

    pub async fn start_listener(&self) {
        info!("EndOfWatch: Listener active on koad:stream:system");
        let topics = vec!["system".to_string()];
        let _ = self
            .signal_corps
            .ensure_consumer_groups("cass", &topics)
            .await;

        loop {
            match self
                .signal_corps
                .read_messages("cass", &topics, Some(1), Some(5000))
                .await
            {
                Ok(messages) => {
                    for (topic, entry_id, fields) in messages {
                        let payload = fields.get("payload").cloned().unwrap_or_default();
                        if let Ok(event) = serde_json::from_str::<serde_json::Value>(&payload) {
                            if event["event_type"] == "session_closed" {
                                self.process_session_close(&event).await;
                            }
                        }
                        let _ = self.signal_corps.ack("cass", &topic, &[entry_id]).await;
                    }
                }
                Err(e) => {
                    error!("EndOfWatch: Stream read error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }
    }

    async fn process_session_close(&self, event: &serde_json::Value) {
        let session_id = event["session_id"].as_str().unwrap_or_default();
        let agent_name = event["agent_name"].as_str().unwrap_or_default();

        info!(session_id = %session_id, agent = %agent_name, "EndOfWatch: Starting distillation");

        // 1. Fetch historical record (In a real implementation, we'''d read the HISTFILE from the bay)
        // For now, we simulate the text to be summarized.
        let raw_history = format!("Agent {} completed Phase 2 of the Citadel Rebuild. Implemented CASS memory services and Signal Corps monitors.", agent_name);

        // 2. Perform Real Distillation via Intelligence Layer
        let summary = match self.intelligence.summarize(&raw_history).await {
            Ok(s) => s,
            Err(e) => {
                warn!(
                    "EndOfWatch: Inference failed, falling back to placeholder. Error: {}",
                    e
                );
                format!(
                    "Session closed for agent {}. [Inference Failed]",
                    agent_name
                )
            }
        };

        // 3. Record as Episodic Memory
        let episode = EpisodicMemory {
            session_id: session_id.to_string(),
            project_path: "unknown".to_string(),
            summary,
            turn_count: 0,
            timestamp: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            task_ids: vec![],
            metadata: None,
        };

        if let Err(e) = self.storage.record_episode(episode).await {
            error!("EndOfWatch: Failed to record episode: {}", e);
        } else {
            info!(session_id = %session_id, "EndOfWatch: Distillation complete.");
        }
    }
}
