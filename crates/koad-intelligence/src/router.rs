//! Inference Routing & Task Management

use koad_core::intelligence::IntelligenceRouter;
use crate::clients::OllamaClient;
use crate::InferenceClient;
use anyhow::Result;
use std::sync::Arc;
use tracing::{info, warn};

/// High-level categories for intelligence tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InferenceTask {
    /// Summarization and history distillation (Local preferred).
    Distillation,
    /// Significance scoring and fact extraction (Local preferred).
    Evaluation,
    /// Complex multi-step reasoning or technical architecture (Cloud preferred).
    Reasoning,
}

/// A router that selects the appropriate [`InferenceClient`] based on task and availability.
pub struct InferenceRouter {
    local_client: Arc<dyn InferenceClient>,
    // cloud_client: Option<Arc<GeminiClient>>, // Reserved for Phase 3.5
}

impl InferenceRouter {
    /// Create a new router with the specified clients.
    pub fn new(local_client: Arc<dyn InferenceClient>) -> Self {
        Self { local_client }
    }

    /// Create a new router with default clients (Local Ollama).
    ///
    /// Model resolved from `KOADOS_INTEL_MODEL` env var, falling back to `"mistral"`.
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be built.
    pub fn new_default() -> Result<Self> {
        let model = std::env::var("KOADOS_INTEL_MODEL").unwrap_or_else(|_| "mistral".to_string());
        info!(model = %model, "InferenceRouter: Initializing with Ollama client.");
        Ok(Self::new(Arc::new(OllamaClient::new(Some(&model), None)?)))
    }

    /// Select a client for the given task.
    /// Currently defaults to Ollama for all tasks until Gemini is wired.
    pub fn select(&self, _task: InferenceTask) -> Arc<dyn InferenceClient> {
        self.local_client.clone()
    }

    /// Convenience: Route a summarization request.
    /// Falls back to returning the original text if the model is unavailable.
    pub async fn summarize(&self, text: &str) -> Result<String> {
        match self.select(InferenceTask::Distillation).summarize(text).await {
            Ok(summary) => Ok(summary),
            Err(e) => {
                warn!("Inference unavailable for summarize ({}), returning raw text.", e);
                Ok(text.to_string())
            }
        }
    }

    /// Convenience: Route a significance scoring request.
    /// Falls back to 1.0 (store everything) if the model is unavailable.
    pub async fn score(&self, text: &str) -> Result<f32> {
        match self.select(InferenceTask::Evaluation).score_significance(text).await {
            Ok(score) => Ok(score),
            Err(e) => {
                warn!("Inference unavailable for score ({}), defaulting to 1.0.", e);
                Ok(1.0)
            }
        }
    }
}

#[async_trait::async_trait]
impl IntelligenceRouter for InferenceRouter {
    async fn summarize(&self, text: &str) -> Result<String> {
        self.summarize(text).await
    }
    async fn analyze(&self, text: &str) -> Result<String> {
        self.analyze(text).await
    }
}
