use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub method: String,
    pub params: Option<Value>,
    #[serde(default)]
    pub id: Option<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<Value>,
    pub id: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema", alias = "input_schema")]
    pub input_schema: Value,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolList {
    pub tools: Vec<McpTool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct McpToolCallResponse {
    pub content: Vec<McpContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
}

pub struct McpServer {
    name: String,
    version: String,
    tools: HashMap<String, Box<dyn McpToolHandler + Send + Sync>>,
}

#[async_trait::async_trait]
pub trait McpToolHandler {
    fn definition(&self) -> McpTool;
    async fn call(&self, params: Value) -> Result<McpToolCallResponse>;
}

impl McpServer {
    pub fn new(name: &str, version: &str) -> Self {
        Self {
            name: name.to_string(),
            version: version.to_string(),
            tools: HashMap::new(),
        }
    }

    pub fn register_tool<T: McpToolHandler + 'static + Send + Sync>(&mut self, handler: T) {
        let def = handler.definition();
        self.tools.insert(def.name.clone(), Box::new(handler));
    }

    pub async fn run(&self) -> Result<()> {
        let mut lines = BufReader::new(tokio::io::stdin()).lines();
        let mut stdout = tokio::io::stdout();

        while let Some(line) = lines.next_line().await? {
            let req: JsonRpcRequest = match serde_json::from_str(&line) {
                Ok(req) => req,
                Err(e) => {
                    tracing::error!("Failed to parse request: {}", e);
                    continue;
                }
            };

            let response = self.handle_request(req).await;
            let res_str = serde_json::to_string(&response)?;
            stdout.write_all(res_str.as_bytes()).await?;
            stdout.write_all(b"\n").await?;
            stdout.flush().await?;
        }

        Ok(())
    }

    pub async fn handle_request(&self, req: JsonRpcRequest) -> JsonRpcResponse {
        let result = match req.method.as_str() {
            "initialize" => {
                let client_version = req.params.as_ref()
                    .and_then(|p| p.get("protocolVersion"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("2024-11-05");
                Some(serde_json::json!({
                    "protocolVersion": client_version,
                    "capabilities": {},
                    "serverInfo": {
                        "name": self.name,
                        "version": self.version,
                    }
                }))
            }
            "tools/list" => {
                let tools: Vec<McpTool> = self.tools.values().map(|h| h.definition()).collect();
                Some(serde_json::to_value(McpToolList { tools }).unwrap())
            }
            "tools/call" => {
                if let Some(params) = req.params {
                    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or_default();
                    let args = params.get("arguments").cloned().unwrap_or(Value::Object(Default::default()));
                    
                    if let Some(handler) = self.tools.get(name) {
                        match handler.call(args).await {
                            Ok(res) => Some(serde_json::to_value(res).unwrap()),
                            Err(e) => Some(serde_json::json!({
                                "content": [{
                                    "type": "text",
                                    "text": format!("Error: {}", e)
                                }],
                                "isError": true
                            })),
                        }
                    } else {
                        Some(serde_json::json!({
                            "content": [{
                                "type": "text",
                                "text": format!("Tool not found: {}", name)
                            }],
                            "isError": true
                        }))
                    }
                } else {
                    Some(serde_json::json!({
                        "content": [{
                            "type": "text",
                            "text": "Missing params for tools/call"
                        }],
                        "isError": true
                    }))
                }
            }
            _ => None,
        };

        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            result: result.clone(),
            error: if result.is_none() {
                Some(serde_json::json!({
                    "code": -32601,
                    "message": format!("Method not found: {}", req.method)
                }))
            } else {
                None
            },
            id: req.id.unwrap_or(Value::Null),
        }
    }
}
