//! Token estimation + budget scoring shared by hydration and metadata enrichment.

use koad_proto::cass::v1::{FactCard, TokenEstimate};

/// Estimate tokens for `text`, labelled with the tokenizer/method used.
pub fn estimate_tokens(text: &str) -> TokenEstimate {
    let tokens = koad_core::utils::tokens::count_tokens(text) as u32;
    TokenEstimate {
        tokenizer: "cl100k_base".to_string(),
        model_hint: String::new(),
        tokens,
        method: "library".to_string(),
        computed_at_unix: chrono::Utc::now().timestamp(),
    }
}

/// Pure token count (no proto wrapper) for hot-path budget math.
pub fn count(text: &str) -> u32 {
    koad_core::utils::tokens::count_tokens(text) as u32
}

/// Value-per-token packing score. Higher = inject sooner.
/// score = confidence * salience * recency_weight / max(1, tokens)
pub fn packing_score(fact: &FactCard) -> f32 {
    let tokens = count(&fact.content).max(1) as f32;
    let (salience, recency) = fact
        .metadata
        .as_ref()
        .and_then(|m| m.retrieval.as_ref())
        .map(|r| {
            let s = if r.salience > 0.0 { r.salience } else { fact.confidence };
            let rec = if r.recency_weight > 0.0 { r.recency_weight } else { 1.0 };
            (s, rec)
        })
        .unwrap_or((fact.confidence, 1.0));
    (fact.confidence * salience * recency) / tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use koad_proto::cass::v1::{MemoryMetadata, RetrievalMetadata};

    #[test]
    fn test_estimate_labels_library() {
        let est = estimate_tokens("hello world");
        assert_eq!(est.method, "library");
        assert_eq!(est.tokenizer, "cl100k_base");
        assert!(est.tokens >= 1);
    }

    #[test]
    fn test_empty_text_is_zero_tokens() {
        assert_eq!(estimate_tokens("").tokens, 0);
    }

    #[test]
    fn test_packing_prefers_concise_high_value() {
        let short = FactCard { content: "short".into(), confidence: 0.9, ..Default::default() };
        let long = FactCard { content: "word ".repeat(200), confidence: 0.9, ..Default::default() };
        assert!(packing_score(&short) > packing_score(&long));
    }

    #[test]
    fn test_salience_overrides_confidence_when_set() {
        let f = FactCard {
            content: "x".into(),
            confidence: 0.5,
            metadata: Some(MemoryMetadata {
                retrieval: Some(RetrievalMetadata { salience: 1.0, recency_weight: 1.0, ..Default::default() }),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert!(packing_score(&f) > 0.5 * 0.5);
    }
}
