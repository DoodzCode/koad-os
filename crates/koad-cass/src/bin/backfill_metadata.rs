//! Backfill default MemoryMetadata for fact_cards lacking it.
//! Usage: backfill_metadata --db <path> [--partition <prefix>] [--apply]
//! Default is dry-run: reports counts + estimated token total, writes nothing.

use anyhow::Result;
use rusqlite::Connection;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let db = arg(&args, "--db").ok_or_else(|| anyhow::anyhow!("--db <path> is required"))?;
    let partition = arg(&args, "--partition");
    let apply = args.iter().any(|a| a == "--apply");

    let conn = Connection::open(&db)?;
    // Idempotent: ensure the column exists even on an older DB.
    let _ = conn.execute("ALTER TABLE fact_cards ADD COLUMN metadata_json TEXT", []);

    let where_clause = match &partition {
        Some(_) => "WHERE metadata_json IS NULL AND (domain = ?1 OR domain LIKE ?1 || ':%')".to_string(),
        None => "WHERE metadata_json IS NULL".to_string(),
    };

    // Collect candidate rows into an owned Vec before `stmt` drops.
    let rows: Vec<(String, String, String, f64, String)> = {
        let sql = format!("SELECT id, domain, content, confidence, source_agent FROM fact_cards {where_clause}");
        let mut stmt = conn.prepare(&sql)?;
        let mapped = |r: &rusqlite::Row| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, f64>(3)?,
                r.get::<_, String>(4)?,
            ))
        };
        match &partition {
            Some(p) => stmt.query_map([p], mapped)?.collect::<rusqlite::Result<Vec<_>>>()?,
            None => stmt.query_map([], mapped)?.collect::<rusqlite::Result<Vec<_>>>()?,
        }
    };

    let total_tokens: u64 = rows
        .iter()
        .map(|(_, _, content, _, _)| koad_core::utils::tokens::count_tokens(content) as u64)
        .sum();
    println!("rows missing metadata: {}", rows.len());
    println!("estimated total tokens: {}", total_tokens);

    if !apply {
        println!("dry-run: no writes. Re-run with --apply to backfill.");
        return Ok(());
    }

    let tx = conn.unchecked_transaction()?;
    for (id, domain, content, confidence, source_agent) in &rows {
        let json = koad_cass::default_metadata_json(content, domain, *confidence as f32, source_agent);
        tx.execute(
            "UPDATE fact_cards SET metadata_json = ?1 WHERE id = ?2",
            rusqlite::params![json, id],
        )?;
    }
    tx.commit()?;
    println!("backfilled {} rows.", rows.len());
    Ok(())
}

fn arg(args: &[String], key: &str) -> Option<String> {
    args.iter().position(|a| a == key).and_then(|i| args.get(i + 1)).cloned()
}
