//! # KoadOS Intelligence (L4 Cognition)
//!
//! This crate provides the "Brain" interface for the Citadel. It abstracts LLM
//! interactions into a unified [`InferenceClient`] trait and provides a task-based
//! [`InferenceRouter`] to balance between local (Ollama) and cloud (Gemini) providers.
//!
//! ## Core Components
//! - **InferenceClient**: The primary trait for chat, summarization, and scoring.
//! - **OllamaClient**: Local-first implementation targeting the Ollama API.
//! - **InferenceRouter**: Orchestrates task delegation based on priority and cost.

use anyhow::Result;

use async_trait::async_trait;

pub mod clients;
pub mod router;

#[cfg(test)]
mod tests;

/// The core interface for all intelligence-backed operations.
#[async_trait]
pub trait InferenceClient: Send + Sync {
    /// Send a raw prompt to the model and receive the response string.
    async fn chat(&self, prompt: &str) -> Result<String>;

    /// Summarize the provided text for context distillation.
    async fn summarize(&self, text: &str) -> Result<String>;

    /// Score the significance of a piece of content (0.0 to 1.0).
    async fn score_significance(&self, text: &str) -> Result<f32>;

    /// Generate a dense vector embedding for the provided text.
    async fn embed(&self, text: &str) -> Result<Vec<f32>>;
}
