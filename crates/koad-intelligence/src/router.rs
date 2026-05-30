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

    /// Convenience: Generate a dense vector embedding using the evaluation client.
    /// Falls back to Gemini or OpenRouter APIs if the local client fails and the appropriate environment variables are configured.
    pub async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        match self.select(InferenceTask::Evaluation).embed(text).await {
            Ok(vec) => Ok(vec),
            Err(e) => {
                warn!("Ollama embedding failed ({:?}). Attempting cloud fallback...", e);
                if let Ok(key) = std::env::var("GEMINI_API_KEY") {
                    match Self::embed_gemini(text, &key).await {
                        Ok(vec) => {
                            info!("Gemini cloud embedding fallback successful.");
                            return Ok(vec);
                        }
                        Err(err) => warn!("Gemini cloud embedding fallback failed: {:?}", err),
                    }
                }
                if let Ok(key) = std::env::var("OPENROUTER_API_KEY") {
                    match Self::embed_openrouter(text, &key).await {
                        Ok(vec) => {
                            info!("OpenRouter cloud embedding fallback successful.");
                            return Ok(vec);
                        }
                        Err(err) => warn!("OpenRouter cloud embedding fallback failed: {:?}", err),
                    }
                }
                Err(e)
            }
        }
    }

    async fn embed_gemini(text: &str, api_key: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/text-embedding-004:embedContent?key={}",
            api_key
        );
        let resp = client
            .post(&url)
            .json(&serde_json::json!({
                "content": {
                    "parts": [{ "text": text }]
                }
            }))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("Gemini API error status: {}", resp.status()));
        }
        let body: serde_json::Value = resp.json().await?;
        let values = body["embedding"]["values"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid Gemini embedding response structure"))?;
        let vec: Result<Vec<f32>, _> = values
            .iter()
            .map(|v| v.as_f64().map(|f| f as f32).ok_or_else(|| anyhow::anyhow!("Invalid float value")))
            .collect();
        vec
    }

    async fn embed_openrouter(text: &str, api_key: &str) -> Result<Vec<f32>> {
        let client = reqwest::Client::new();
        let url = "https://openrouter.ai/api/v1/embeddings";
        let resp = client
            .post(url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&serde_json::json!({
                "model": "text-embedding-3-small",
                "input": text
            }))
            .send()
            .await?;
        if !resp.status().is_success() {
            return Err(anyhow::anyhow!("OpenRouter API error status: {}", resp.status()));
        }
        let body: serde_json::Value = resp.json().await?;
        let values = body["data"][0]["embedding"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("Invalid OpenRouter embedding response structure"))?;
        let vec: Result<Vec<f32>, _> = values
            .iter()
            .map(|v| v.as_f64().map(|f| f as f32).ok_or_else(|| anyhow::anyhow!("Invalid float value")))
            .collect();
        vec
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
