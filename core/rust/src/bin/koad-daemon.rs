use notify::{Watcher, RecursiveMode, Config, Event};
use std::path::{Path, PathBuf};
use rusqlite::{params, Connection};
use dirs;
use std::fs::OpenOptions;
use std::io::{Write, Read};
use std::sync::{Arc, Mutex};
use std::process::{Command, Stdio};
use chrono::Local;

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
        
        let _ = conn.execute("PRAGMA busy_timeout = 5000", []);

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

        // Ensure command_queue exists
        conn.execute(
            "CREATE TABLE IF NOT EXISTS command_queue (
                id INTEGER PRIMARY KEY,
                command TEXT NOT NULL,
                args TEXT,
                status TEXT NOT NULL DEFAULT 'pending',
                output TEXT,
                pid INTEGER,
                created_at TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT
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
            let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
            let _ = writeln!(file, "[{}] {}", now, msg);
        }
    }

    fn log_change(&self, path: &Path, event_type: &str) -> std::result::Result<(), anyhow::Error> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        let path_str = path.to_string_lossy().to_string();
        
        if path_str.contains(".git/") || path_str.contains("target/") || path_str.contains("daemon.log") || path_str.contains(".koad-os") {
            return Ok(());
        }

        self.debug_log(&format!("Event: {} on {}", event_type, path_str));

        let conn = self.db_conn.lock().unwrap();
        conn.execute(
            "INSERT INTO project_state (path, event_type, timestamp) VALUES (?1, ?2, ?3)",
            params![path_str, event_type, now],
        )?;
        
        Ok(())
    }

    fn process_queue(&self) -> std::result::Result<(), anyhow::Error> {
        let mut pending = Vec::new();
        {
            let conn = self.db_conn.lock().unwrap();
            let mut stmt = conn.prepare("SELECT id, command, args FROM command_queue WHERE status = 'pending'")?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, Option<String>>(2)?))
            })?;
            for row in rows {
                pending.push(row?);
            }
        }

        for (id, cmd, args) in pending {
            let daemon_clone = Arc::new(Self {
                db_conn: Arc::clone(&self.db_conn),
                log_path: self.log_path.clone(),
            });
            
            std::thread::spawn(move || {
                let _ = daemon_clone.execute_task(id, cmd, args);
            });
        }

        Ok(())
    }

    fn record_delta(&self, category: &str, content: &str) -> std::result::Result<(), anyhow::Error> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        self.debug_log(&format!("Recording Delta: {} - {}", category, content));
        let conn = self.db_conn.lock().unwrap();
        match conn.execute(
            "INSERT INTO knowledge (category, content, tags, timestamp) VALUES (?1, ?2, 'delta,spine', ?3)",
            params!["learning", format!("[SPINE] {}: {}", category, content), now],
        ) {
            Ok(_) => {
                self.debug_log("Delta recorded successfully.");
                Ok(())
            },
            Err(e) => {
                self.debug_log(&format!("Failed to record delta: {}", e));
                Err(anyhow::anyhow!(e))
            }
        }
    }

    fn execute_task(&self, id: i64, cmd: String, args_str: Option<String>) -> std::result::Result<(), anyhow::Error> {
        let now = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        {
            let conn = self.db_conn.lock().unwrap();
            conn.execute("UPDATE command_queue SET status = 'running', started_at = ?1 WHERE id = ?2", params![now, id])?;
        }

        self.debug_log(&format!("Task {}: Starting {} with args {:?}", id, cmd, args_str));

        let mut command = Command::new(&cmd);
        if let Some(a) = args_str {
            for arg in a.split_whitespace() {
                command.arg(arg);
            }
        }
        
        let output = command
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn();

        match output {
            Ok(child) => {
                let pid = child.id() as i64;
                {
                    let conn = self.db_conn.lock().unwrap();
                    conn.execute("UPDATE command_queue SET pid = ?1 WHERE id = ?2", params![pid, id])?;
                }

                let output = child.wait_with_output()?;
                
                let final_out = format!("STDOUT:\n{}\n\nSTDERR:\n{}", 
                                        String::from_utf8_lossy(&output.stdout), 
                                        String::from_utf8_lossy(&output.stderr));
                let end_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let status_str = if output.status.success() { "completed" } else { "failed" };

                let conn = self.db_conn.lock().unwrap();
                conn.execute("UPDATE command_queue SET status = ?1, output = ?2, finished_at = ?3 WHERE id = ?4", 
                             params![status_str, final_out, end_time, id])?;
                
                self.debug_log(&format!("Task {}: Finished with status {}", id, status_str));
                let _ = self.record_delta("TaskComplete", &format!("Task #{} ({}) finished as {}", id, cmd, status_str));
            },
            Err(e) => {
                let end_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
                let conn = self.db_conn.lock().unwrap();
                conn.execute("UPDATE command_queue SET status = 'failed', output = ?1, finished_at = ?2 WHERE id = ?3", 
                             params![format!("Spawn Error: {}", e), end_time, id])?;
                self.debug_log(&format!("Task {}: Failed to spawn: {}", id, e));
                let _ = self.record_delta("TaskError", &format!("Task #{} ({}) failed: {}", id, cmd, e));
            }
        }

        Ok(())
    }
}

fn main() -> anyhow::Result<()> {
    println!("Initializing Koad Daemon...");
    let daemon = Arc::new(Daemon::init().map_err(|e| anyhow::anyhow!(e.to_string()))?);
    println!("Daemon initialized. Starting services...");
    daemon.debug_log("--- Daemon Started (Spine Mode) ---");
    
    // Command Queue Polling Thread
    let daemon_poll = Arc::clone(&daemon);
    std::thread::spawn(move || {
        println!("Command dispatcher active.");
        loop {
            if let Err(e) = daemon_poll.process_queue() {
                daemon_poll.debug_log(&format!("Queue Error: {:?}", e));
            }
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    });

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

    println!("Setting up filesystem watcher... (DISABLED for mission realignment)");
    // Realignment: Only watch registered projects
    /*
    let mut watched_paths = Vec::new();
...
    }
    */

    println!("Koad Spine is alive. (Passive Monitoring Only)");
    // Keep the main thread alive
    loop {
        std::thread::sleep(std::time::Duration::from_secs(60));
    }
}
