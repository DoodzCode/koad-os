//! Memory Service Implementation

use crate::storage::MemoryTier;
use crate::token_budget::estimate_tokens;
use koad_intelligence::router::InferenceRouter;
use koad_proto::cass::v1::memory_service_server::MemoryService;
use koad_proto::cass::v1::{
    EpisodicMemory, FactCard, FactQuery, FactResponse, MemoryMetadata, PromptBudgetHints,
    ProvenanceMetadata, RetrievalMetadata, SemanticQuery,
};
use koad_proto::citadel::v5::StatusResponse;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use tracing::info;

const STABLE_TOPICS: &[&str] = &["identity", "profile", "project", "convention", "user"];

fn topic_of(domain: &str) -> &str {
    domain.rsplit(':').next().unwrap_or(domain)
}

/// Pure default-metadata derivation. Backfills only empty subfields of `existing`
/// (pass `None` for a fresh card). Never returns None.
pub fn default_metadata(
    content: &str,
    domain: &str,
    confidence: f32,
    source_agent: &str,
    existing: Option<MemoryMetadata>,
) -> MemoryMetadata {
    let tokens = estimate_tokens(content);
    let n = tokens.tokens;
    let topic = topic_of(domain);
    let stable = STABLE_TOPICS.contains(&topic);

    let mut md = existing.unwrap_or_default();

    if md.token_estimates.is_empty() {
        md.token_estimates.push(tokens);
    }

    let pb = md.prompt_budget.get_or_insert_with(PromptBudgetHints::default);
    if pb.priority.is_empty() {
        pb.priority = "normal".into();
    }
    if pb.injection_mode.is_empty() {
        pb.injection_mode = if n <= 80 {
            "verbatim".into()
        } else if !md.summary.is_empty() {
            "summary".into()
        } else {
            "verbatim".into()
        };
    }
    if !pb.cache_stable {
        pb.cache_stable = stable;
    }
    // allow_compression intentionally left at proto3 default: bool can't distinguish unset from explicit-false, so we don't force a default here (revisit if proto adopts optional fields).

    let rt = md.retrieval.get_or_insert_with(RetrievalMetadata::default);
    // proto3 limitation: an explicit 0.0 is indistinguishable from unset, so an explicit salience/recency of 0.0 is treated as unset and backfilled. Accepted for v1.
    if rt.salience == 0.0 {
        rt.salience = confidence;
    }
    if rt.recency_weight == 0.0 {
        rt.recency_weight = 1.0;
    }
    if rt.volatility.is_empty() {
        rt.volatility = if stable { "stable".into() } else { "mutable".into() };
    }

    let pv = md.provenance.get_or_insert_with(ProvenanceMetadata::default);
    if pv.content_hash.is_empty() {
        let mut h = Sha256::new();
        h.update(content.as_bytes());
        pv.content_hash = format!("{:x}", h.finalize());
    }
    if pv.schema_version.is_empty() {
        pv.schema_version = "1".into();
    }
    if pv.author_agent.is_empty() {
        pv.author_agent = source_agent.to_string();
    }

    md
}

/// JSON form for backfill reuse.
pub fn default_metadata_json(
    content: &str,
    domain: &str,
    confidence: f32,
    source_agent: &str,
) -> String {
    serde_json::to_string(&default_metadata(content, domain, confidence, source_agent, None))
        .unwrap_or_default()
}

pub struct CassMemoryService {
    storage: Arc<dyn MemoryTier>,
    intelligence: Arc<InferenceRouter>,
}

impl CassMemoryService {
    pub fn new(storage: Arc<dyn MemoryTier>, intelligence: Arc<InferenceRouter>) -> Self {
        Self {
            storage,
            intelligence,
        }
    }
}

#[tonic::async_trait]
impl MemoryService for CassMemoryService {
    async fn commit_fact(
        &self,
        request: Request<FactCard>,
    ) -> Result<Response<StatusResponse>, Status> {
        let mut fact = request.into_inner();
        info!(domain = %fact.domain, "Memory: Committing fact");
        fact.metadata = Some(default_metadata(
            &fact.content,
            &fact.domain,
            fact.confidence,
            &fact.source_agent,
            fact.metadata.take(),
        ));

        self.storage
            .commit_fact(fact)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(StatusResponse {
            success: true,
            message: "Fact committed to ledger.".to_string(),
            context: None,
        }))
    }

    async fn query_facts(
        &self,
        request: Request<FactQuery>,
    ) -> Result<Response<FactResponse>, Status> {
        let req = request.into_inner();
        let facts = self
            .storage
            .query_facts(&req.domain, &req.tags, req.limit)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(FactResponse { facts }))
    }

    async fn record_episode(
        &self,
        request: Request<EpisodicMemory>,
    ) -> Result<Response<StatusResponse>, Status> {
        let mut ep = request.into_inner();
        info!(session = %ep.session_id, "Memory: Recording episode");

        {
            let mut md = ep.metadata.take().unwrap_or_default();
            if md.token_estimates.is_empty() {
                md.token_estimates
                    .push(crate::token_budget::estimate_tokens(&ep.summary));
            }
            ep.metadata = Some(md);
        }

        // Intelligence: Extract facts from summary
        let summary = ep.summary.clone();
        let intelligence = self.intelligence.clone();
        tokio::spawn(async move {
            let _ = intelligence.score(&summary).await;
            // Future: auto-extract FactCards
        });

        self.storage
            .record_episode(ep)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(StatusResponse {
            success: true,
            message: "Episode recorded.".to_string(),
            context: None,
        }))
    }

    async fn search_semantic(
        &self,
        request: Request<SemanticQuery>,
    ) -> Result<Response<FactResponse>, Status> {
        let req = request.into_inner();
        let facts = self
            .storage
            .search_semantic(&req.query, &req.partition, req.limit)
            .await
            .map_err(|e| Status::internal(e.to_string()))?;

        Ok(Response::new(FactResponse { facts }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::mock::MockStorage;
    use koad_intelligence::router::InferenceRouter;
    use koad_proto::cass::v1::FactCard;
    use std::sync::Arc;
    use tonic::Request;

    fn svc_and_storage() -> (CassMemoryService, Arc<MockStorage>) {
        let storage = Arc::new(MockStorage::new());
        let svc = CassMemoryService::new(
            storage.clone(),
            Arc::new(InferenceRouter::new_default().unwrap()),
        );
        (svc, storage)
    }

    #[tokio::test]
    async fn test_commit_auto_populates_defaults() {
        let (s, storage) = svc_and_storage();
        let fact = FactCard {
            id: "auto-1".into(),
            domain: "hermes_jupiter_ideans:identity".into(),
            content: "Ian is Dood (admin/approver).".into(),
            confidence: 1.0,
            ..Default::default()
        };
        s.commit_fact(Request::new(fact)).await.unwrap();
        let got = storage.query_facts("", &[], 10).await.unwrap();
        let md = got[0].metadata.as_ref().expect("metadata auto-populated");
        assert_eq!(md.token_estimates.len(), 1);
        assert_eq!(md.prompt_budget.as_ref().unwrap().priority, "normal");
        assert!(md.prompt_budget.as_ref().unwrap().cache_stable); // identity ⇒ stable
        assert_eq!(md.retrieval.as_ref().unwrap().volatility, "stable");
        assert!(!md.provenance.as_ref().unwrap().content_hash.is_empty());
    }

    #[tokio::test]
    async fn test_commit_preserves_explicit_metadata() {
        let (s, storage) = svc_and_storage();
        let mut fact = FactCard {
            id: "auto-2".into(),
            content: "x".into(),
            confidence: 1.0,
            ..Default::default()
        };
        fact.metadata = Some(MemoryMetadata {
            prompt_budget: Some(PromptBudgetHints {
                priority: "critical".into(),
                allow_compression: false,
                ..Default::default()
            }),
            retrieval: Some(RetrievalMetadata {
                salience: 0.7,
                recency_weight: 0.5,
                ..Default::default()
            }),
            ..Default::default()
        });
        s.commit_fact(Request::new(fact)).await.unwrap();
        let got = storage.query_facts("", &[], 10).await.unwrap();
        let md = got[0].metadata.as_ref().unwrap();
        let pb = md.prompt_budget.as_ref().unwrap();
        assert_eq!(pb.priority, "critical"); // explicit kept
        assert!(!pb.allow_compression); // explicit false survives (not force-defaulted)
        let rt = md.retrieval.as_ref().unwrap();
        assert_eq!(rt.salience, 0.7); // explicit non-zero survives
        assert_eq!(rt.recency_weight, 0.5); // explicit non-zero survives
        assert_eq!(md.token_estimates.len(), 1); // missing subfield backfilled
    }
}
