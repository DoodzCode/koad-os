use notify::{Watcher, RecursiveMode, Result};
use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;
use rusqlite::{params, Connection};
use dirs;

struct Daemon {
    db_conn: Connection,
}

impl Daemon {
    fn init() -> std::result::Result<Self, anyhow::Error> {
        let home = std::env::var("KOAD_HOME")
            .map(PathBuf::from)
            .or_else(|_| dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Home dir not found")).map(|h| h.join(".koad-os")))?;
        
        let db_path = home.join("koad.db");
        let conn = Connection::open(db_path)?;
        
        // Ensure the project_state table exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS project_state (
                id INTEGER PRIMARY KEY,
                path TEXT NOT NULL,
                event_type TEXT NOT NULL,
                timestamp TEXT NOT NULL,
                summary TEXT
            )",
            [],
        )?;
        
        Ok(Self { db_conn: conn })
    }

    fn log_change(&self, path: &Path, event_type: &str) -> std::result::Result<(), anyhow::Error> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let path_str = path.to_string_lossy().to_string();
        
        // Filter out noise
        if path_str.contains(".git/") || path_str.contains("target/") || path_str.contains(".koad-os") {
            return Ok(());
        }

        self.db_conn.execute(
            "INSERT INTO project_state (path, event_type, timestamp) VALUES (?1, ?2, ?3)",
            params![path_str, event_type, now],
        )?;
        
        println!("[DAEMON] Logged {} on {}", event_type, path_str);
        Ok(())
    }
}

fn main() -> Result<()> {
    println!("--- Koad Cognitive Booster Daemon Starting ---");
    
    // Fix: map_err using a reference
    let daemon = Daemon::init().map_err(|e| notify::Error::generic(&e.to_string()))?;
    let (tx, rx) = channel();

    // Watch current directory
    let mut watcher = notify::RecommendedWatcher::new(tx, notify::Config::default())?;
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    
    println!("[DAEMON] Watching project at: {}", current_dir.display());
    watcher.watch(&current_dir, RecursiveMode::Recursive)?;

    // Event Loop
    for res in rx {
        match res {
            Ok(event) => {
                if event.kind.is_modify() || event.kind.is_create() || event.kind.is_remove() {
                    for path in event.paths {
                        let _ = daemon.log_change(&path, &format!("{:?}", event.kind));
                    }
                }
            },
            Err(e) => println!("watch error: {:?}", e),
        }
    }

    Ok(())
}
