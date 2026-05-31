//! One-shot CASS ingestion tool.
//! Reads a markdown file, splits on `## ` headers, commits each chunk as a FactCard.
//! Does NOT require KOAD_SESSION_ID or an active Citadel session.
//!
//! Usage: cass-ingest <file.md> --domain <domain> --agent <agent> [--tags tag1,tag2] [--dry-run]

use anyhow::{Context, Result};
use chrono::Utc;
use clap::Parser;
use koad_proto::cass::v1::memory_service_client::MemoryServiceClient;
use koad_proto::cass::v1::{EpisodicMemory, FactCard, FactQuery};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "cass-ingest", about = "Ingest markdown session files into CASS")]
struct Args {
    /// Markdown file to ingest
    file: PathBuf,

    /// CASS gRPC URL
    #[arg(long, default_value = "http://localhost:50052")]
    cass_url: String,

    /// Domain / partition key for all committed facts
    #[arg(long, default_value = "session")]
    domain: String,

    /// Source agent name
    #[arg(long, default_value = "clyde")]
    agent: String,

    /// Session ID (free string, no Citadel needed)
    #[arg(long, default_value = "clyde-2026-05-09")]
    session_id: String,

    /// Comma-separated tags
    #[arg(long, default_value = "")]
    tags: String,

    /// Print chunks without committing
    #[arg(long)]
    dry_run: bool,

    /// Also record an EpisodicMemory entry for the whole file
    #[arg(long)]
    record_episode: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    let content = std::fs::read_to_string(&args.file)
        .with_context(|| format!("Cannot read {:?}", args.file))?;

    let tags: Vec<String> = args
        .tags
        .split(',')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();

    let chunks = split_markdown(&content);
    println!("[cass-ingest] {} chunks from {:?}", chunks.len(), args.file);

    if args.dry_run {
        for (i, (title, body)) in chunks.iter().enumerate() {
            println!("\n--- chunk {} ---\nTITLE: {}\nBODY: {}...", i + 1, title, &body[..body.len().min(120)]);
        }
        return Ok(());
    }

    let mut client = MemoryServiceClient::connect(args.cass_url.clone())
        .await
        .context("Cannot connect to CASS")?;

    let mut committed = 0usize;
    for (title, body) in &chunks {
        let chunk_content = format!("### {title}\n\n{body}");
        let mut hasher = Sha256::new();
        hasher.update(chunk_content.as_bytes());
        let id = format!("{:x}", hasher.finalize());

        let fact = FactCard {
            id,
            source_agent: args.agent.clone(),
            session_id: args.session_id.clone(),
            domain: args.domain.clone(),
            content: chunk_content,
            confidence: 1.0,
            tags: tags.clone(),
            created_at: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
        };

        match client.commit_fact(fact).await {
            Ok(r) => {
                let inner = r.into_inner();
                if inner.success {
                    println!("[OK] {title}");
                    committed += 1;
                } else {
                    println!("[FAIL] {title}: {}", inner.message);
                }
            }
            Err(e) => println!("[ERR] {title}: {e}"),
        }
    }

    if args.record_episode {
        let episode = EpisodicMemory {
            session_id: args.session_id.clone(),
            project_path: std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| ".".to_string()),
            summary: content[..content.len().min(2000)].to_string(),
            turn_count: chunks.len() as u32,
            timestamp: Some(prost_types::Timestamp {
                seconds: Utc::now().timestamp(),
                nanos: 0,
            }),
            task_ids: vec![],
        };

        match client.record_episode(episode).await {
            Ok(_) => println!("[OK] Episode recorded"),
            Err(e) => println!("[ERR] Episode: {e}"),
        }
    }

    println!("\n[cass-ingest] Done: {committed}/{} facts committed to domain '{}'", chunks.len(), args.domain);
    Ok(())
}

fn split_markdown(content: &str) -> Vec<(String, String)> {
    let mut chunks = Vec::new();
    let mut current_title = "preamble".to_string();
    let mut current_body = String::new();

    for line in content.lines() {
        if line.starts_with("## ") {
            if !current_body.trim().is_empty() {
                chunks.push((current_title.clone(), current_body.trim().to_string()));
            }
            current_title = line.trim_start_matches("## ").to_string();
            current_body = String::new();
        } else {
            current_body.push_str(line);
            current_body.push('\n');
        }
    }

    if !current_body.trim().is_empty() {
        chunks.push((current_title, current_body.trim().to_string()));
    }

    chunks
}
