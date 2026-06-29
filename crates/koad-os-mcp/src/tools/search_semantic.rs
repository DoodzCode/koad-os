use anyhow::Result;
use async_trait::async_trait;
use koad_mcp::{McpContent, McpTool, McpToolCallResponse, McpToolHandler};
use koad_proto::cass::v1::memory_service_client::MemoryServiceClient;
use koad_proto::cass::v1::SemanticQuery;
use serde_json::{json, Value};

pub struct SearchSemanticTool {
    cass_url: String,
    partition: String,
}

impl SearchSemanticTool {
    pub fn new(cass_url: String, partition: String) -> Self {
        Self { cass_url, partition }
    }
}

#[async_trait]
impl McpToolHandler for SearchSemanticTool {
    fn definition(&self) -> McpTool {
        McpTool {
            name: "memory.search_semantic".to_string(),
            description: "Search memory cards by content relevance".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {
                        "type": "string",
                        "description": "Search query"
                    },
                    "limit": {
                        "type": "integer",
                        "description": "Max results (default 5)",
                        "default": 5
                    },
                    "include_metadata": {
                        "type": "boolean",
                        "description": "Append a compact metadata line per card (token estimate, priority, injection mode, salience, volatility, sensitivity). Default false to keep output lean.",
                        "default": false
                    }
                },
                "required": ["query"]
            }),
        }
    }

    async fn call(&self, params: Value) -> Result<McpToolCallResponse> {
        let query = params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as u32;
        let include_metadata = params
            .get("include_metadata")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let mut client = MemoryServiceClient::connect(self.cass_url.clone()).await?;
        let resp = client
            .search_semantic(SemanticQuery {
                query,
                partition: self.partition.clone(),
                limit,
                min_score: 0.0,
            })
            .await?
            .into_inner();

        let text = if resp.facts.is_empty() {
            "No matching memory cards found.".to_string()
        } else {
            resp.facts
                .iter()
                .map(|f| {
                    let mut line = format!("**[{}]** {}\n_{}_", f.domain, f.content, f.id);
                    if include_metadata {
                        if let Some(md) = f.metadata.as_ref() {
                            line.push('\n');
                            line.push_str(&crate::tools::render_metadata_compact(md));
                        }
                    }
                    line
                })
                .collect::<Vec<_>>()
                .join("\n\n---\n\n")
        };

        Ok(McpToolCallResponse {
            content: vec![McpContent::Text { text }],
            is_error: None,
        })
    }
}
