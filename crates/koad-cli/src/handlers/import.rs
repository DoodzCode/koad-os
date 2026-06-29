use crate::cli::ImportRoute;
use crate::handlers::system::spawn_issue;
use anyhow::{Context, Result};
use chrono::Utc;
use koad_core::config::KoadConfig;
use koad_proto::cass::v1::memory_service_client::MemoryServiceClient;
use koad_proto::cass::v1::FactCard;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[allow(clippy::too_many_arguments)]
pub async fn handle_import(
    source: PathBuf,
    format: String,
    delimiter: Option<String>,
    route: ImportRoute,
    template: Option<String>,
    labels: Vec<String>,
    dry_run: bool,
    config: &KoadConfig,
    db: &koad_core::db::KoadDB,
) -> Result<()> {
    println!(
        ">>> [IMPORT] Energizing Ingestion Pipeline: {}...",
        source.display()
    );

    let content = std::fs::read_to_string(&source)
        .with_context(|| format!("Failed to read source file: {:?}", source))?;

    let chunks = match format.as_str() {
        "md" => {
            let delim = delimiter.unwrap_or_else(|| r"^## (?:!|\d+\.)".to_string());
            parse_markdown_chunks(&content, &delim)?
        }
        _ => anyhow::bail!("Unsupported import format: {}", format),
    };

    println!(">>> Found {} potential payloads.", chunks.len());

    let mut client = if matches!(route, ImportRoute::Hydration) {
        Some(MemoryServiceClient::connect(config.network.cass_grpc_addr.clone()).await?)
    } else {
        None
    };

    // For hydration, we need the active session ID.
    // In a real scenario, this would be fetched from the local environment/boot state.
    // For now, we'll use a placeholder or look it up.
    let session_id =
        std::env::var("KOAD_SESSION_ID").unwrap_or_else(|_| "admin-session".to_string());

    for (i, (title, body)) in chunks.into_iter().enumerate() {
        if dry_run {
            println!("\n--- [DRY RUN] Payload {} ---", i + 1);
            println!("TITLE: {}", title);
            println!(
                "BODY Snippet: {}...",
                body.chars()
                    .take(100)
                    .collect::<String>()
                    .replace("\n", " ")
            );
            continue;
        }

        match route {
            ImportRoute::GithubIssues => {
                let tmpl = template.as_deref().unwrap_or("feature");
                println!("[SYNC] Spawning Issue: {}...", title);
                spawn_issue(
                    config,
                    db,
                    tmpl,
                    &title,
                    "standard",
                    None,
                    None,
                    labels.clone(),
                    Some(body),
                )
                .await?;
            }
            ImportRoute::Hydration => {
                let mut hasher = Sha256::new();
                hasher.update(&body);
                let chunk_id = format!("{:x}", hasher.finalize());

                println!("[SYNC] Hydrating Context Chunk: {}...", title);
                if let Some(c) = &mut client {
                    let request = FactCard {
                        id: chunk_id,
                        source_agent: "admiral".to_string(),
                        session_id: session_id.clone(),
                        domain: "import".to_string(),
                        content: format!("### {}\n{}", title, body),
                        confidence: 1.0,
                        tags: labels.clone(),
                        created_at: Some(prost_types::Timestamp {
                            seconds: Utc::now().timestamp(),
                            nanos: Utc::now().timestamp_subsec_nanos() as i32,
                        }),
                        metadata: None,
                    };
                    let response = c.commit_fact(request).await?.into_inner();
                    if !response.success {
                        println!("  \x1b[31m[ERROR]\x1b[0m {}", response.message);
                    } else {
                        println!("  \x1b[32m[OK]\x1b[0m Fact committed to memory bank.");
                    }
                }
            }
            _ => anyhow::bail!("Route {:?} not yet implemented.", route),
        }
    }

    if dry_run {
        println!("\n>>> [CONDITION GREEN] Dry-run complete. No data was persisted.");
    } else {
        println!("\n>>> [CONDITION GREEN] Ingestion complete. Pipeline stable.");
    }

    Ok(())
}

fn parse_markdown_chunks(content: &str, delimiter: &str) -> Result<Vec<(String, String)>> {
    let re = regex::RegexBuilder::new(delimiter)
        .multi_line(true)
        .build()
        .context("Invalid delimiter regex")?;

    let mut results = Vec::new();

    // Find all delimiter matches and their end positions
    let matches: Vec<_> = re.find_iter(content).collect();
    if matches.is_empty() {
        return Ok(vec![]);
    }

    for i in 0..matches.len() {
        let start = matches[i].end();
        let end = if i + 1 < matches.len() {
            matches[i + 1].start()
        } else {
            content.len()
        };

        let chunk = &content[start..end].trim();
        if chunk.is_empty() {
            continue;
        }

        let mut lines = chunk.lines();
        if let Some(first_line) = lines.next() {
            let mut title = first_line.trim().to_string();
            // GitHub Title Limit is 256 chars. Truncate to 250 for safety.
            if title.len() > 250 {
                title = format!("{}...", &title[..247]);
            }
            let body = lines.collect::<Vec<_>>().join("\n").trim().to_string();
            if !title.is_empty() {
                results.push((title, body));
            }
        }
    }

    Ok(results)
}
