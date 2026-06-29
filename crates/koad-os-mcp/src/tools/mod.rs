pub mod commit;
pub mod intel_get;
pub mod list_topics;
pub mod recall;
pub mod search_semantic;
pub mod status;

use koad_proto::cass::v1::MemoryMetadata;

/// Compact one-line metadata summary for debug output in recall/search.
pub fn render_metadata_compact(md: &MemoryMetadata) -> String {
    let toks = md.token_estimates.first().map(|t| t.tokens).unwrap_or(0);
    let (prio, mode) = md
        .prompt_budget
        .as_ref()
        .map(|p| (p.priority.clone(), p.injection_mode.clone()))
        .unwrap_or_default();
    let (sal, vol) = md
        .retrieval
        .as_ref()
        .map(|r| (r.salience, r.volatility.clone()))
        .unwrap_or((0.0, String::new()));
    let sens = md
        .privacy
        .as_ref()
        .map(|p| p.sensitivity.clone())
        .unwrap_or_default();
    format!("    └ meta: ~{toks}tok prio={prio} mode={mode} sal={sal:.2} vol={vol} sens={sens}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use koad_proto::cass::v1::{
        MemoryMetadata, PromptBudgetHints, RetrievalMetadata, PrivacyMetadata, TokenEstimate,
    };

    #[test]
    fn test_render_metadata_compact_full() {
        let md = MemoryMetadata {
            token_estimates: vec![TokenEstimate { tokens: 42, ..Default::default() }],
            prompt_budget: Some(PromptBudgetHints { priority: "high".into(), injection_mode: "summary".into(), ..Default::default() }),
            retrieval: Some(RetrievalMetadata { salience: 0.75, volatility: "stable".into(), ..Default::default() }),
            privacy: Some(PrivacyMetadata { sensitivity: "private".into(), ..Default::default() }),
            ..Default::default()
        };
        let s = render_metadata_compact(&md);
        assert!(s.contains("~42tok"));
        assert!(s.contains("prio=high"));
        assert!(s.contains("mode=summary"));
        assert!(s.contains("sal=0.75"));
        assert!(s.contains("vol=stable"));
        assert!(s.contains("sens=private"));
    }

    #[test]
    fn test_render_metadata_compact_empty_no_panic() {
        let s = render_metadata_compact(&MemoryMetadata::default());
        assert!(s.contains("~0tok"));
    }
}
