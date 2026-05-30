//! Ollama Local Inference Client

use crate::InferenceClient;
use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::Client;
use serde_json::json;
use std::time::Duration;
use tracing::{debug, error};

/// A client for the local Ollama API (http://localhost:11434).
pub struct OllamaClient {
    client: Client,
    model: String,
    base_url: String,
}

impl OllamaClient {
    /// Create a new Ollama client with the specified model (default: "mistral").
    ///
    /// # Errors
    /// Returns an error if the HTTP client cannot be built.
    pub fn new(model: Option<&str>, base_url: Option<&str>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .connect_timeout(Duration::from_secs(1))
            .build()
            .context("Failed to build Ollama reqwest client")?;

        Ok(Self {
            client,
            model: model.unwrap_or("mistral").to_string(),
            base_url: base_url.unwrap_or("http://localhost:11434").to_string(),
        })
    }
}

#[async_trait]
impl InferenceClient for OllamaClient {
    async fn chat(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);

        debug!(model = %self.model, "Ollama: Sending request to {}", url);

        let response = self
            .client
            .post(&url)
            .json(&json!({
                "model": self.model,
                "prompt": prompt,
                "stream": false,
                "options": {
                    "num_gpu": 99
                }
            }))
            .send()
            .await
            .context(
                "Failed to connect to Ollama. Ensure it is running at http://localhost:11434.",
            )?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            error!(status = %status, error = %err_text, "Ollama API Error");
            return Err(anyhow::anyhow!("Ollama API returned error: {}", err_text));
        }

        let body: serde_json::Value = response.json().await?;
        Ok(body["response"]
            .as_str()
            .unwrap_or_default()
            .trim()
            .to_string())
    }

    async fn summarize(&self, text: &str) -> Result<String> {
        let prompt = format!(
            "Summarize the following session history for an AI agent. Focus on key decisions, task status, and technical findings. Output ONLY a concise markdown summary.\n\n### TEXT TO SUMMARIZE:\n{}",
            text
        );
        self.chat(&prompt).await
    }

    async fn score_significance(&self, text: &str) -> Result<f32> {
        let prompt = format!(
            "Analyze the following technical content and score its 'significance' for long-term memory on a scale of 0.0 to 1.0. 
            0.0 means noise (errors, greetings, trivialities). 
            1.0 means critical (architectural decisions, resolved bugs, user preferences).
            Output ONLY the numeric score.\n\n### CONTENT:\n{}",
            text
        );

        let response = self.chat(&prompt).await?;
        let score: f32 = response.parse().unwrap_or_else(|_| {
            debug!(
                "Ollama: Failed to parse significance score from '{}', defaulting to 0.5",
                response
            );
            0.5
        });

        Ok(score.clamp(0.0, 1.0))
    }

    async fn embed(&self, text: &str) -> Result<Vec<f32>> {
        let url = format!("{}/api/embeddings", self.base_url);

        debug!(model = %self.model, "Ollama: Sending embeddings request to {}", url);

        let response = self
            .client
            .post(&url)
            .json(&json!({
                "model": self.model,
                "prompt": text,
            }))
            .send()
            .await
            .context("Failed to connect to Ollama embeddings endpoint.")?;

        if !response.status().is_success() {
            let status = response.status();
            let err_text = response.text().await.unwrap_or_default();
            error!(status = %status, error = %err_text, "Ollama Embeddings API Error");
            return Err(anyhow::anyhow!("Ollama Embeddings API returned error: {}", err_text));
        }

        let body: serde_json::Value = response.json().await?;
        let embedding_val = &body["embedding"];
        if let Some(arr) = embedding_val.as_array() {
            let vec: Result<Vec<f32>, _> = arr
                .iter()
                .map(|v| v.as_f64().map(|f| f as f32).context("Invalid embedding element"))
                .collect();
            vec
        } else {
            Err(anyhow::anyhow!("Ollama response did not contain 'embedding' array"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    #[tokio::test]
    async fn chat_returns_trimmed_response_field() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"response": "  hello world  "})),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::new(None, Some(&server.uri())).unwrap();
        let reply = client.chat("test prompt").await.unwrap();
        assert_eq!(
            reply, "hello world",
            "Response should have leading/trailing whitespace trimmed"
        );
    }

    #[tokio::test]
    async fn chat_returns_error_on_http_error_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal server error"))
            .mount(&server)
            .await;

        let client = OllamaClient::new(None, Some(&server.uri())).unwrap();
        let result = client.chat("test").await;
        assert!(result.is_err(), "Non-2xx response should produce an error");
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Ollama API returned error"),
            "Error message should describe the API failure"
        );
    }

    #[tokio::test]
    async fn score_significance_falls_back_to_0_5_on_non_numeric_response() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(serde_json::json!({"response": "not-a-number"})),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::new(None, Some(&server.uri())).unwrap();
        let score = client.score_significance("some content").await.unwrap();
        assert_eq!(
            score, 0.5,
            "Should fall back to 0.5 when the response cannot be parsed as f32"
        );
    }

    #[tokio::test]
    async fn score_significance_parses_valid_float() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"response": "0.75"})),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::new(None, Some(&server.uri())).unwrap();
        let score = client
            .score_significance("important content")
            .await
            .unwrap();
        assert_eq!(score, 0.75);
    }

    #[tokio::test]
    async fn score_significance_clamps_above_1_to_1() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"response": "1.9"})),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::new(None, Some(&server.uri())).unwrap();
        let score = client.score_significance("content").await.unwrap();
        assert_eq!(score, 1.0, "Score above 1.0 should be clamped to 1.0");
    }

    #[tokio::test]
    async fn score_significance_clamps_negative_to_0() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/generate"))
            .respond_with(
                ResponseTemplate::new(200).set_body_json(serde_json::json!({"response": "-0.5"})),
            )
            .mount(&server)
            .await;

        let client = OllamaClient::new(None, Some(&server.uri())).unwrap();
        let score = client.score_significance("content").await.unwrap();
        assert_eq!(score, 0.0, "Negative score should be clamped to 0.0");
    }
}
