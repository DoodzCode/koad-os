use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use koad_mcp::{McpContent, McpTool, McpToolCallResponse, McpToolHandler};
use koad_proto::cass::v1::memory_service_client::MemoryServiceClient;
use koad_proto::cass::v1::FactCard;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub struct CommitTool {
    cass_url: String,
    partition: String,
    agent_name: String,
}

impl CommitTool {
    pub fn new(cass_url: String, partition: String, agent_name: String) -> Self {
        Self { cass_url, partition, agent_name }
    }
}

#[async_trait]
impl McpToolHandler for CommitTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "memory.commit".to_string(),
            description: "Save a memory card to persistent storage. Use this to record important \
                facts, decisions, or context that should be recalled in future sessions. \
                Each commit is deduplicated by content hash — committing the same content twice \
                is safe and idempotent."
                .to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The fact or context to remember. Be specific and self-contained — \
                            this card may be read in a future session without surrounding context."
                    },
                    "topic": {
                        "type": "string",
                        "description": "Short topic label for this memory (e.g. 'network-outage', \
                            'user-kimmie', 'onboarding-checklist'). Used for filtering and recall. \
                            Lowercase, hyphens OK. Defaults to 'general'.",
                        "default": "general"
                    },
                    "tags": {
                        "type": "array",
                        "items": { "type": "string" },
                        "description": "Optional list of tags for filtering (e.g. [\"skylinks\", \"incident\"])",
                        "default": []
                    },
                    "confidence": {
                        "type": "number",
                        "description": "Confidence score 0.0–1.0. Use 1.0 for confirmed facts, \
                            lower for inferred or uncertain information. Default 1.0.",
                        "default": 1.0
                    }
                },
                "required": ["content"]
            }),
        }
    }

    async fn call(&self, params: Value) -> Result<McpToolCallResponse> {
        let content = match params.get("content").and_then(|v| v.as_str()) {
            Some(c) if !c.trim().is_empty() => c.trim().to_string(),
            _ => {
                return Ok(McpToolCallResponse {
                    content: vec![McpContent::Text {
                        text: "Error: content is required and cannot be empty.".to_string(),
                    }],
                    is_error: Some(true),
                });
            }
        };

        let topic = params
            .get("topic")
            .and_then(|v| v.as_str())
            .unwrap_or("general")
            .to_lowercase();

        let tags: Vec<String> = params
            .get("tags")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|t| t.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let confidence = params
            .get("confidence")
            .and_then(|v| v.as_f64())
            .unwrap_or(1.0)
            .clamp(0.0, 1.0) as f32;

        // Content-hash dedup: same content always produces same ID
        let mut hasher = Sha256::new();
        hasher.update(format!("{}:{}", self.partition, content).as_bytes());
        let id = format!("{:x}", hasher.finalize());

        // Domain = partition + topic for scoped recall
        let domain = format!("{}:{}", self.partition, topic);

        let session_id = std::env::var("KOAD_SESSION_ID")
            .unwrap_or_else(|_| format!("rook-{}", Utc::now().format("%Y%m%d")));

        let fact = FactCard {
            id: id.clone(),
            source_agent: self.agent_name.clone(),
            session_id,
            domain: domain.clone(),
            content: content.clone(),
            confidence,
            tags,
            created_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
        };

        let mut client = MemoryServiceClient::connect(self.cass_url.clone()).await?;
        let resp = client.commit_fact(fact).await?.into_inner();

        let text = if resp.success {
            format!(
                "Memory saved.\n\nID: {}\nDomain: {}\nContent: {}",
                &id[..12],
                domain,
                &content[..content.len().min(200)]
            )
        } else {
            format!("Commit failed: {}", resp.message)
        };

        Ok(McpToolCallResponse {
            content: vec![McpContent::Text { text }],
            is_error: if resp.success { None } else { Some(true) },
        })
    }
}
