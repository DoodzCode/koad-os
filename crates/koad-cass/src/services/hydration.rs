//! # Hydration Service Implementation
//!
//! Performs the Temporal Context Hydration (TCH) walk to bundle
//! relevant facts and episodes for agent boot.
//! This service integrates structural code maps and intelligent history
//! distillation into a single, high-density markdown packet.

use crate::storage::{MemoryTier, PulseTier};
use koad_codegraph::CodeGraph;
use koad_core::hierarchy::HierarchyManager;
use koad_core::utils::tokens::count_tokens;
use koad_core::intelligence::IntelligenceRouter;
#[cfg(test)]
use koad_intelligence::router::InferenceRouter;
use koad_proto::cass::v1::hydration_service_server::HydrationService;
use koad_proto::cass::v1::{HydrationRequest, HydrationResponse};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::info;

/// Service implementation for the `HydrationService` gRPC interface.
pub struct CassHydrationService {
    storage: Arc<dyn MemoryTier>,
    hierarchy: Arc<HierarchyManager>,
    codegraph: Arc<CodeGraph>,
    intelligence: Arc<dyn IntelligenceRouter>,
    pulse_store: Option<Arc<dyn PulseTier>>,
}

impl CassHydrationService {
    /// Creates a new `CassHydrationService`.
    pub fn new(
        storage: Arc<dyn MemoryTier>,
        hierarchy: Arc<HierarchyManager>,
        codegraph: Arc<CodeGraph>,
        intelligence: Arc<dyn IntelligenceRouter>,
    ) -> Self {
        Self {
            storage,
            hierarchy,
            codegraph,
            intelligence,
            pulse_store: None,
        }
    }

    /// Attaches an optional pulse store for Global Pulses section in TCH.
    pub fn with_pulse_store(mut self, store: Arc<dyn PulseTier>) -> Self {
        self.pulse_store = Some(store);
        self
    }
}

#[tonic::async_trait]
impl HydrationService for CassHydrationService {
    /// Bundles context for an agent based on their workspace level and token budget.
    ///
    /// # Errors
    /// Returns a `tonic::Status` if storage queries or intelligence distillation fail.
    async fn hydrate(
        &self,
        request: Request<HydrationRequest>,
    ) -> Result<Response<HydrationResponse>, Status> {
        let req = request.into_inner();
        let current_path = PathBuf::from(&req.project_root);
        let budget = req.token_budget as usize;
        let agent = &req.agent_name;
        let task_id = if req.task_id.is_empty() {
            None
        } else {
            Some(req.task_id.as_str())
        };

        info!(agent = %agent, path = %req.project_root, budget = %budget, task = ?task_id, "TCH: Hydration requested");

        let mut packet = format!(
            "# Temporal Context Hydration: {}\nDate: {}\n\n",
            agent,
            chrono::Utc::now().format("%Y-%m-%d")
        );
        let mut tokens_used = count_tokens(&packet);
        let mut source_files = Vec::new();

        // 0. Identity Anchor (New)
        if let Some(id_config) = self.hierarchy.config().identities.get(&agent.to_lowercase()) {
            let mut identity_section = "## ⚓ Identity Anchor\n".to_string();
            identity_section.push_str(&format!("- **Name:** {}\n", id_config.name));
            identity_section.push_str(&format!("- **Role:** {}\n", id_config.role));
            identity_section.push_str(&format!("- **Rank:** {}\n", id_config.rank));
            identity_section.push_str(&format!("- **Bio:** {}\n", id_config.bio));
            
            if let Some(pref) = &id_config.preferences {
                if !pref.principles.is_empty() {
                    identity_section.push_str("\n### Core Principles\n");
                    for p in &pref.principles {
                        identity_section.push_str(&format!("- {}\n", p));
                    }
                }
            }
            identity_section.push_str("\n");

            let section_tokens = count_tokens(&identity_section);
            if tokens_used + section_tokens < budget {
                packet.push_str(&identity_section);
                tokens_used += section_tokens;
            }
        }

        // 0.1 Project Brief (New)
        let brief_path = current_path.join("agents/CITADEL.md");
        if brief_path.exists() {
            if let Ok(content) = fs::read_to_string(&brief_path) {
                let brief_section = format!("## 📋 Project Brief\n{}\n\n", content);
                let section_tokens = count_tokens(&brief_section);
                if tokens_used + section_tokens < budget {
                    packet.push_str(&brief_section);
                    tokens_used += section_tokens;
                }
            }
        }

        // 1. Agent History Distillation (Upgrade 2 + 3)
        let episodes = self
            .storage
            .query_recent_episodes(agent, 5, task_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if !episodes.is_empty() {
            let mut raw_history = String::new();
            for ep in &episodes {
                raw_history.push_str(&format!("Session {}: {}\n", ep.session_id, ep.summary));
            }

            // Use precomputed episodic summaries directly to avoid on-the-fly LLM synthesis
            let mut summary_block = "## Ⅰ. Recent Episode Summaries (Distilled History)\n".to_string();
            for ep in &episodes {
                summary_block.push_str(&format!("- Session {}: {}\n", ep.session_id, ep.summary));
            }
            summary_block.push_str("\n");
            let ep_section = summary_block;

            let section_tokens = count_tokens(&ep_section);
            if tokens_used + section_tokens < budget {
                packet.push_str(&ep_section);
                tokens_used += section_tokens;
            }
        }

        // 2. High-Signal Facts (Upgrade 3)
        let facts = self
            .storage
            .query_agent_facts(agent, 10, task_id)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        if !facts.is_empty() {
            use crate::token_budget::{count, packing_score};

            fn priority_rank(f: &koad_proto::cass::v1::FactCard) -> u8 {
                match f
                    .metadata
                    .as_ref()
                    .and_then(|m| m.prompt_budget.as_ref())
                    .map(|p| p.priority.as_str())
                {
                    Some("critical") => 0,
                    Some("high") => 1,
                    Some("normal") | None => 2,
                    Some("low") => 3,
                    Some("archive") => 4,
                    Some(_) => 2,
                }
            }

            let mut ranked = facts;
            ranked.sort_by(|a, b| {
                priority_rank(a).cmp(&priority_rank(b)).then(
                    packing_score(b)
                        .partial_cmp(&packing_score(a))
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
            });

            let header = "## Ⅱ. Active Fact Cards\n";
            let mut body = String::new();
            for fact in &ranked {
                let mode = fact
                    .metadata
                    .as_ref()
                    .and_then(|m| m.prompt_budget.as_ref())
                    .map(|p| p.injection_mode.as_str())
                    .unwrap_or("verbatim");
                if mode == "never_auto" {
                    continue;
                }
                let text = match mode {
                    "title_only" => format!("- [{}] (Conf: {:.2})\n", fact.domain, fact.confidence),
                    "summary" => {
                        let s = fact
                            .metadata
                            .as_ref()
                            .map(|m| m.summary.as_str())
                            .filter(|s| !s.is_empty());
                        format!(
                            "- [{}] (Conf: {:.2}): {}\n",
                            fact.domain,
                            fact.confidence,
                            s.unwrap_or(&fact.content)
                        )
                    }
                    _ => format!(
                        "- [{}] (Conf: {:.2}): {}\n",
                        fact.domain, fact.confidence, fact.content
                    ),
                };
                let line_tokens = count(&text) as usize;
                if tokens_used + (count(header) as usize) + (count(&body) as usize) + line_tokens
                    >= budget
                {
                    continue;
                }
                body.push_str(&text);
            }
            if !body.is_empty() {
                let section = format!("{header}{body}\n");
                let section_tokens = count(&section) as usize;
                if tokens_used + section_tokens < budget {
                    packet.push_str(&section);
                    tokens_used += section_tokens;
                }
            }
        }

        // 2.5 Pending Inbox (New)
        let inbox_dir = current_path.join("agents/inbox");
        if inbox_dir.is_dir() {
            if let Ok(entries) = fs::read_dir(inbox_dir) {
                let mut inbox_section = "## 📨 Pending Inbox\n".to_string();
                let mut found_messages = false;
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("md") {
                        let filename = path.file_name().and_then(|s| s.to_str()).unwrap_or("");
                        // Check if message is for this agent or 'all'
                        if filename.contains(&agent.to_lowercase()) || filename.contains("all") {
                            if let Ok(content) = fs::read_to_string(&path) {
                                inbox_section.push_str(&format!("### Message: {}\n{}\n\n", filename, content));
                                found_messages = true;
                            }
                        }
                    }
                }
                if found_messages {
                    let section_tokens = count_tokens(&inbox_section);
                    if tokens_used + section_tokens < budget {
                        packet.push_str(&inbox_section);
                        tokens_used += section_tokens;
                    }
                }
            }
        }

        // 3. Hierarchy Walk (Layers)
        let mut layers = Vec::new();
        let mut temp_path = current_path.clone();
        for _ in 0..5 {
            let agents_dir = temp_path.join("agents");
            if agents_dir.is_dir() {
                layers.push(temp_path.clone());
            }
            if let Some(parent) = temp_path.parent() {
                temp_path = parent.to_path_buf();
                if current_path == Path::new("/") {
                    break;
                }
            } else {
                break;
            }
        }

        if !layers.is_empty() {
            let home_dir = std::env::var("HOME").unwrap_or_default();
            let mut hierarchy_section = "## Ⅲ. Workspace Hierarchy\n".to_string();
            for layer in layers.iter().rev() {
                let level = self.hierarchy.resolve_level(layer);
                let display_path = layer.to_string_lossy();
                let sanitized = if !home_dir.is_empty() {
                    display_path.replacen(&home_dir, "~", 1)
                } else {
                    display_path.into_owned()
                };
                hierarchy_section.push_str(&format!("### Level: {:?}\nPath: {}\n\n", level, sanitized));
                source_files.push(layer.to_string_lossy().to_string());
            }

            let section_tokens = count_tokens(&hierarchy_section);
            if tokens_used + section_tokens < budget {
                packet.push_str(&hierarchy_section);
                tokens_used += section_tokens;
            }
        }

        // 4. Ghost API Summaries (Upgrade 1)
        let mut api_header = "## Ⅳ. Crate API Maps (Ghost Summaries)\n".to_string();
        api_header.push_str("The following public items are available in your current workspace members. Use these to find symbols without reading files.\n");

        let header_tokens = count_tokens(&api_header);
        if tokens_used + header_tokens < budget {
            packet.push_str(&api_header);
            tokens_used += header_tokens;

            // We'll summarize the current crate and core
            let crate_list = vec![
                current_path.to_string_lossy().to_string(),
                current_path
                    .join("crates/koad-core")
                    .to_string_lossy()
                    .to_string(),
            ];

            for c_path in crate_list {
                if let Ok(summary) = self.codegraph.get_crate_summary(&c_path) {
                    if !summary.is_empty() {
                        let c_name = Path::new(&c_path)
                            .file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("unknown");
                        let block = format!("\n### Crate: {}\n{}\n", c_name, summary);
                        let block_tokens = count_tokens(&block);
                        if tokens_used + block_tokens < budget {
                            packet.push_str(&block);
                            tokens_used += block_tokens;
                        }
                    }
                }
            }
        }

        // 5. Global Pulses
        if let Some(pulse_store) = &self.pulse_store {
            let agent_role = "global"; // In future, derive from agent identity
            if let Ok(pulses) = pulse_store.get_active_pulses(agent_role).await {
                if !pulses.is_empty() {
                    let mut pulse_section = "\n## Ⅴ. Global Pulses\n".to_string();
                    for p in &pulses {
                        pulse_section
                            .push_str(&format!("- [{}] {}: {}\n", p.role, p.author, p.message));
                    }
                    let section_tokens = count_tokens(&pulse_section);
                    if tokens_used + section_tokens < budget {
                        packet.push_str(&pulse_section);
                        tokens_used += section_tokens;
                    }
                }
            }
        }

        Ok(Response::new(HydrationResponse {
            markdown_packet: packet,
            estimated_tokens: tokens_used as u32,
            source_files,
        }))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::mock::{MockPulseStore, MockStorage};
    use koad_proto::cass::v1::Pulse;

    #[tokio::test]
    async fn test_hydration_bundles_sections() -> anyhow::Result<()> {
        let storage = Arc::new(MockStorage::new());
        let config = koad_core::config::KoadConfig::load().unwrap_or_else(|_| {
            koad_core::config::KoadConfig::from_json(
                r#"{
                "home": "/tmp",
                "system": { "version": "test" },
                "network": { "citadel_grpc_port": 0, "citadel_grpc_addr": "", "cass_grpc_port": 0, "cass_grpc_addr": "", "redis_socket": "", "citadel_socket": "" },
                "storage": { "db_name": "", "drain_interval_secs": 0 }
            }"#,
            )
            .unwrap()
        });
        let hierarchy = Arc::new(HierarchyManager::new(config));
        let codegraph = Arc::new(CodeGraph::new_with_memory()?);
        let intelligence = Arc::new(InferenceRouter::new_default()?);

        let service = CassHydrationService::new(storage, hierarchy, codegraph, intelligence);

        let request = Request::new(HydrationRequest {
            agent_name: "test-agent".to_string(),
            project_root: "/tmp".to_string(),
            level: 0,
            token_budget: 10000,
            task_id: "".to_string(),
        });

        let response = service.hydrate(request).await?;
        let packet = response.into_inner().markdown_packet;

        assert!(packet.contains("# Temporal Context Hydration"));
        Ok(())
    }

    #[tokio::test]
    async fn test_hydration_includes_pulse_section() -> anyhow::Result<()> {
        let storage = Arc::new(MockStorage::new());
        let config = koad_core::config::KoadConfig::load().unwrap_or_else(|_| {
            koad_core::config::KoadConfig::from_json(
                r#"{
                "home": "/tmp",
                "system": { "version": "test" },
                "network": { "citadel_grpc_port": 0, "citadel_grpc_addr": "", "cass_grpc_port": 0, "cass_grpc_addr": "", "redis_socket": "", "citadel_socket": "" },
                "storage": { "db_name": "", "drain_interval_secs": 0 }
            }"#,
            )
            .unwrap()
        });
        let hierarchy = Arc::new(HierarchyManager::new(config));
        let codegraph = Arc::new(CodeGraph::new_with_memory()?);
        let intelligence = Arc::new(InferenceRouter::new_default()?);

        let pulse_store = Arc::new(MockPulseStore::new());
        pulse_store
            .seed(Pulse {
                id: "test-pulse-1".to_string(),
                author: "test-agent".to_string(),
                role: "global".to_string(),
                message: "Test pulse active".to_string(),
                ttl_seconds: 3600,
                created_at: None,
            })
            .await;

        let service = CassHydrationService::new(storage, hierarchy, codegraph, intelligence)
            .with_pulse_store(pulse_store);

        let request = Request::new(HydrationRequest {
            agent_name: "test-agent".to_string(),
            project_root: "/tmp".to_string(),
            level: 0,
            token_budget: 10000,
            task_id: "".to_string(),
        });

        let response = service.hydrate(request).await?;
        let packet = response.into_inner().markdown_packet;

        assert!(
            packet.contains("Global Pulses"),
            "TCH packet missing Global Pulses section"
        );
        assert!(
            packet.contains("Test pulse active"),
            "TCH packet missing pulse message"
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_hydration_prefers_concise_high_value_under_budget() -> anyhow::Result<()> {
        use koad_proto::cass::v1::FactCard;
        let storage = Arc::new(MockStorage::new());
        storage
            .commit_fact(FactCard {
                id: "concise".into(),
                source_agent: "test-agent".into(),
                domain: "p:identity".into(),
                content: "Key fact alpha.".into(),
                confidence: 1.0,
                ..Default::default()
            })
            .await?;
        storage
            .commit_fact(FactCard {
                id: "verbose".into(),
                source_agent: "test-agent".into(),
                domain: "p:trivia".into(),
                content: format!("{} zulu", "filler ".repeat(400)),
                confidence: 0.3,
                ..Default::default()
            })
            .await?;

        let config = koad_core::config::KoadConfig::load().unwrap_or_else(|_| {
            koad_core::config::KoadConfig::from_json(
                r#"{
                "home": "/tmp",
                "system": { "version": "test" },
                "network": { "citadel_grpc_port": 0, "citadel_grpc_addr": "", "cass_grpc_port": 0, "cass_grpc_addr": "", "redis_socket": "", "citadel_socket": "" },
                "storage": { "db_name": "", "drain_interval_secs": 0 }
            }"#,
            )
            .unwrap()
        });
        let hierarchy = Arc::new(HierarchyManager::new(config));
        let codegraph = Arc::new(CodeGraph::new_with_memory()?);
        let intelligence = Arc::new(InferenceRouter::new_default()?);

        let service = CassHydrationService::new(storage, hierarchy, codegraph, intelligence);

        // Budget large enough for the TCH header + fact header + the concise fact,
        // but far too small for the ~400-token verbose fact.
        let request = Request::new(HydrationRequest {
            agent_name: "test-agent".to_string(),
            project_root: "/tmp".to_string(),
            level: 0,
            token_budget: 60,
            task_id: "".to_string(),
        });

        let response = service.hydrate(request).await?;
        let packet = response.into_inner().markdown_packet;

        assert!(
            packet.contains("Key fact alpha."),
            "concise high-value fact should be packed under budget"
        );
        assert!(
            !packet.contains("filler filler"),
            "verbose low-value fact should be dropped under budget"
        );
        Ok(())
    }
}
