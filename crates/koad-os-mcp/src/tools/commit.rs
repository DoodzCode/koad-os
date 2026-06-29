use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use koad_mcp::{McpContent, McpTool, McpToolCallResponse, McpToolHandler};
use koad_proto::cass::v1::memory_service_client::MemoryServiceClient;
use koad_proto::cass::v1::FactCard;
use koad_proto::cass::v1::{MemoryMetadata, PromptBudgetHints, RetrievalMetadata, PrivacyMetadata};
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
                    },
                    "summary": { "type": "string", "description": "Short alternate injection text for long cards." },
                    "priority": { "type": "string", "enum": ["critical","high","normal","low","archive"], "description": "Prompt-packing priority. Default normal." },
                    "injection_mode": { "type": "string", "enum": ["verbatim","summary","title_only","never_auto"], "description": "How this card is injected during hydration." },
                    "max_prompt_tokens": { "type": "integer", "minimum": 0, "description": "Hard cap on tokens when this card is injected." },
                    "preferred_prompt_tokens": { "type": "integer", "minimum": 0, "description": "Target token budget for this card." },
                    "salience": { "type": "number", "minimum": 0.0, "maximum": 1.0, "description": "Durable usefulness 0..1. Defaults to confidence." },
                    "volatility": { "type": "string", "enum": ["stable","mutable","ephemeral"], "description": "How fast this fact goes stale." },
                    "sensitivity": { "type": "string", "enum": ["public","internal","private","secret-adjacent"], "description": "Privacy class for prompt-packing." }
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
            metadata: parse_metadata(&params),
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

/// Build optional metadata from the MCP params. Returns None when no metadata
/// fields were provided (the CASS service backfills defaults).
pub(crate) fn parse_metadata(params: &Value) -> Option<MemoryMetadata> {
    let mut md = MemoryMetadata::default();
    let mut any = false;

    let mut pb = PromptBudgetHints::default();
    let mut have_pb = false;
    if let Some(s) = params.get("priority").and_then(|v| v.as_str()) { pb.priority = s.into(); have_pb = true; }
    if let Some(s) = params.get("injection_mode").and_then(|v| v.as_str()) { pb.injection_mode = s.into(); have_pb = true; }
    if let Some(n) = params.get("max_prompt_tokens").and_then(|v| v.as_u64()) { pb.max_prompt_tokens = n as u32; have_pb = true; }
    if let Some(n) = params.get("preferred_prompt_tokens").and_then(|v| v.as_u64()) { pb.preferred_prompt_tokens = n as u32; have_pb = true; }
    if have_pb { md.prompt_budget = Some(pb); any = true; }

    let mut rt = RetrievalMetadata::default();
    let mut have_rt = false;
    if let Some(f) = params.get("salience").and_then(|v| v.as_f64()) { rt.salience = f as f32; have_rt = true; }
    if let Some(s) = params.get("volatility").and_then(|v| v.as_str()) { rt.volatility = s.into(); have_rt = true; }
    if have_rt { md.retrieval = Some(rt); any = true; }

    if let Some(s) = params.get("sensitivity").and_then(|v| v.as_str()) {
        md.privacy = Some(PrivacyMetadata { sensitivity: s.into(), ..Default::default() });
        any = true;
    }
    if let Some(s) = params.get("summary").and_then(|v| v.as_str()) { md.summary = s.into(); any = true; }

    if any { Some(md) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_metadata_parse_is_optional() {
        let p = json!({ "content": "hello" });
        assert!(parse_metadata(&p).is_none());
    }

    #[test]
    fn test_metadata_parse_reads_fields() {
        let p = json!({ "content": "hello", "priority": "high", "summary": "s", "salience": 0.8, "sensitivity": "private" });
        let md = parse_metadata(&p).unwrap();
        assert_eq!(md.prompt_budget.as_ref().unwrap().priority, "high");
        assert_eq!(md.summary, "s");
        assert!((md.retrieval.as_ref().unwrap().salience - 0.8).abs() < 1e-6);
        assert_eq!(md.privacy.as_ref().unwrap().sensitivity, "private");
    }
}
