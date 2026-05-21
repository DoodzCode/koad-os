use anyhow::Result;
use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use koad_mcp::{JsonRpcRequest, JsonRpcResponse, McpServer};
use std::{net::SocketAddr, sync::Arc};
use tokio::net::TcpListener;
use tools::commit::CommitTool;
use tools::intel_get::IntelGetTool;
use tools::list_topics::ListTopicsTool;
use tools::recall::RecallTool;
use tools::search_semantic::SearchSemanticTool;
use tools::status::StatusTool;

mod tools;

#[derive(Clone)]
struct AppState {
    server: Arc<McpServer>,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let cass_url = std::env::var("CASS_URL").unwrap_or_else(|_| "http://localhost:50052".to_string());
    let partition = std::env::var("AGENT_PARTITION").unwrap_or_else(|_| "rook_local_default".to_string());
    let mcp_mode = std::env::var("MCP_MODE").unwrap_or_else(|_| "read_only".to_string());
    let transport = std::env::var("MCP_TRANSPORT").unwrap_or_else(|_| "http".to_string());
    let agent_name = std::env::var("AGENT_NAME").unwrap_or_else(|_| "rook".to_string());
    let port: u16 = std::env::var("MCP_PORT")
        .unwrap_or_else(|_| "9742".to_string())
        .parse()
        .unwrap_or(9742);

    tracing::info!(partition = %partition, mode = %mcp_mode, transport = %transport, "Rook MCP starting");

    let mut server = McpServer::new("rook", "0.1.0");
    server.register_tool(RecallTool::new(cass_url.clone(), partition.clone()));
    server.register_tool(SearchSemanticTool::new(cass_url.clone(), partition.clone()));
    server.register_tool(ListTopicsTool::new(cass_url.clone(), partition.clone()));
    server.register_tool(IntelGetTool::new(cass_url.clone(), partition.clone()));
    server.register_tool(StatusTool::new(cass_url.clone(), partition.clone()));

    if mcp_mode == "read_write" {
        tracing::info!("MCP_MODE=read_write: memory.commit tool enabled");
        server.register_tool(CommitTool::new(cass_url.clone(), partition.clone(), agent_name.clone()));
    }

    match transport.as_str() {
        "stdio" => {
            tracing::info!("Transport: stdio");
            server.run().await?;
        }
        _ => {
            let state = AppState { server: Arc::new(server) };
            let app = Router::new()
                .route("/mcp", post(handle_mcp))
                .route("/health", axum::routing::get(|| async { "ok" }))
                .with_state(state);

            let addr = SocketAddr::from(([0, 0, 0, 0], port));
            tracing::info!(%addr, "Transport: HTTP");
            let listener = TcpListener::bind(addr).await?;
            axum::serve(listener, app).await?;
        }
    }

    Ok(())
}

async fn handle_mcp(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>, (StatusCode, String)> {
    let resp = state.server.handle_request(req).await;
    Ok(Json(resp))
}
