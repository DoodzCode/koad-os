use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, List, ListItem, Paragraph},
    Terminal,
};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;
use std::time::{Duration, Instant};
use anyhow::Result;
use crate::KoadDB;

pub fn run_dash(db: &KoadDB) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(3),  // Header
                    Constraint::Min(10),    // Main Content
                    Constraint::Length(3),  // Footer
                ])
                .split(f.size());

            // Header
            let header = Paragraph::new(" KoadOS Real-Time Dashboard - [OBSERVER MODE] ")
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            // Main Body: 3 Columns
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(33),
                    Constraint::Percentage(34),
                    Constraint::Percentage(33),
                ])
                .split(chunks[1]);

            // 1. MIND (Left) - Knowledge & Ponderings
            let mut mind_items: Vec<ListItem> = Vec::new();
            if let Ok(recent) = db.query("", 15, None) {
                for (_, cat, content, _) in recent {
                    let style = if content.contains("[SPINE]") {
                        Style::default().fg(Color::Blue).add_modifier(Modifier::ITALIC)
                    } else {
                        match cat.as_str() {
                            "fact" => Style::default().fg(Color::Cyan),
                            "learning" => Style::default().fg(Color::Green),
                            _ => Style::default().fg(Color::White),
                        }
                    };
                    mind_items.push(ListItem::new(format!("[{}] {}", cat.to_uppercase(), content)).style(style));
                }
            }
            if !mind_items.is_empty() { mind_items.push(ListItem::new("---").style(Style::default().fg(Color::DarkGray))); }
            if let Ok(ponders) = db.get_ponderings(10) {
                for p in ponders {
                    mind_items.push(ListItem::new(format!("> {}", p)).style(Style::default().fg(Color::Magenta)));
                }
            }
            let list = List::new(mind_items)
                .block(Block::default().borders(Borders::ALL).title(" [MIND] Knowledge & Journal "));
            f.render_widget(list, body_chunks[0]);

            // 2. VOICE (Middle) - Notes, Brainstorms, Rants
            let mut voice_items: Vec<ListItem> = Vec::new();
            if let Ok(notes) = db.get_notes(10) {
                for (_, content, _) in notes {
                    voice_items.push(ListItem::new(format!("(Note) {}", content)).style(Style::default().fg(Color::White)));
                }
            }
            if !voice_items.is_empty() { voice_items.push(ListItem::new("---").style(Style::default().fg(Color::DarkGray))); }
            if let Ok(brainstorms) = db.get_recent_brainstorms(10) {
                for (content, cat, _) in brainstorms {
                    let style = if cat == "rant" { Style::default().fg(Color::Red) } else { Style::default().fg(Color::Yellow) };
                    voice_items.push(ListItem::new(format!("[{}] {}", cat.to_uppercase(), content)).style(style));
                }
            }
            let list = List::new(voice_items)
                .block(Block::default().borders(Borders::ALL).title(" [VOICE] Notes & Brainstorms "));
            f.render_widget(list, body_chunks[1]);

            // 3. ACTIVITY (Right) - Daemon & Commands
            let mut activity_items: Vec<ListItem> = Vec::new();
            {
                if let Ok(mut stmt) = db.conn.prepare("SELECT id, command, status FROM command_queue ORDER BY created_at DESC LIMIT 10") {
                    if let Ok(rows) = stmt.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))) {
                        for row in rows {
                            if let Ok((id, cmd, status)) = row {
                                let style = match status.as_str() {
                                    "pending" => Style::default().fg(Color::Yellow),
                                    "running" => Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD),
                                    "completed" => Style::default().fg(Color::Green),
                                    "failed" => Style::default().fg(Color::Red),
                                    _ => Style::default().fg(Color::White),
                                };
                                activity_items.push(ListItem::new(format!("[TASK #{}] {} - {}", id, cmd.to_uppercase(), status)).style(style));
                            }
                        }
                    }
                }
            }
            if !activity_items.is_empty() { activity_items.push(ListItem::new("---").style(Style::default().fg(Color::DarkGray))); }
            if let Ok(deltas) = db.get_recent_deltas(60) {
                for (path, event, _) in deltas {
                    let p = std::path::Path::new(&path);
                    let name = p.file_name().unwrap_or_default().to_string_lossy();
                    activity_items.push(ListItem::new(format!("[DAEMON] {}: {}", event, name)).style(Style::default().fg(Color::Blue)));
                }
            }
            if !activity_items.is_empty() { activity_items.push(ListItem::new("---").style(Style::default().fg(Color::DarkGray))); }
            if let Ok(execs) = db.get_recent_executions(24) {
                for (cmd, args, status) in execs {
                    let style = if status == "success" { Style::default().fg(Color::Green) } else { Style::default().fg(Color::Red) };
                    activity_items.push(ListItem::new(format!("[CLI] {} {}", cmd, args)).style(style));
                }
            }
            let list = List::new(activity_items)
                .block(Block::default().borders(Borders::ALL).title(" [ACTIVITY] Pulse & Events "));
            f.render_widget(list, body_chunks[2]);

            // Footer
            let footer = Paragraph::new(" [q] Exit | [Auto-refresh: 500ms] ")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                if let KeyCode::Char('q') = key.code {
                    break;
                }
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}
