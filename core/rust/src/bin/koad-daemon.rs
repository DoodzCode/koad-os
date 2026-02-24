use notify::{Watcher, RecursiveMode, Config, Event};
use std::path::{Path, PathBuf};
use rusqlite::{params, Connection};
use dirs;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::{Arc, Mutex};

struct Daemon {
    db_conn: Arc<Mutex<Connection>>,
    log_path: PathBuf,
}

impl Daemon {
    fn init() -> std::result::Result<Self, anyhow::Error> {
        let home = std::env::var("KOAD_HOME")
            .map(PathBuf::from)
            .or_else(|_| dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Home dir not found")).map(|h| h.join(".koad-os")))?;
        
        let db_path = home.join("koad.db");
        let log_path = home.join("daemon.log");
        let conn = Connection::open(db_path)?;
        
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
        
        Ok(Self { 
            db_conn: Arc::new(Mutex::new(conn)), 
            log_path 
        })
    }

    fn debug_log(&self, msg: &str) {
        if let Ok(mut file) = OpenOptions::new().append(true).create(true).open(&self.log_path) {
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let _ = writeln!(file, "[{}] {}", now, msg);
        }
    }

    fn log_change(&self, path: &Path, event_type: &str) -> std::result::Result<(), anyhow::Error> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let path_str = path.to_string_lossy().to_string();
        
        self.debug_log(&format!("Event: {} on {}", event_type, path_str));

        if path_str.contains(".git/") || path_str.contains("target/") || path_str.contains("daemon.log") || path_str.contains(".koad-os") {
            return Ok(());
        }

        let conn = self.db_conn.lock().unwrap();
        conn.execute(
            "INSERT INTO project_state (path, event_type, timestamp) VALUES (?1, ?2, ?3)",
            params![path_str, event_type, now],
        )?;
        
        Ok(())
    }
}

fn main() -> notify::Result<()> {
    let daemon = Arc::new(Daemon::init().map_err(|e| notify::Error::generic(&e.to_string()))?);
    daemon.debug_log("--- Daemon Started (Callback Mode) ---");
    
    let daemon_clone = Arc::clone(&daemon);
    let mut watcher = notify::RecommendedWatcher::new(
        move |res: notify::Result<Event>| {
            match res {
                Ok(event) => {
                    let kind_str = format!("{:?}", event.kind);
                    for path in event.paths {
                        let _ = daemon_clone.log_change(&path, &kind_str);
                    }
                },
                Err(e) => daemon_clone.debug_log(&format!("Watch Error: {:?}", e)),
            }
        },
        Config::default(),
    )?;

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    daemon.debug_log(&format!("Watching: {}", current_dir.display()));
    
    watcher.watch(&current_dir, RecursiveMode::Recursive)?;

    // Keep the main thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
