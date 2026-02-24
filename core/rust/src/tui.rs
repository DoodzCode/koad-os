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

            // 1. Knowledge Feed (Left)
            if let Ok(recent) = db.query("") {
                let items: Vec<ListItem> = recent.iter().take(20).map(|(_, cat, content, _)| {
                    let style = match cat.as_str() {
                        "fact" => Style::default().fg(Color::Cyan),
                        "learning" => Style::default().fg(Color::Green),
                        _ => Style::default().fg(Color::White),
                    };
                    ListItem::new(format!("[{}] {}", cat.to_uppercase(), content)).style(style)
                }).collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(" Knowledge Stream "));
                f.render_widget(list, body_chunks[0]);
            }

            // 2. Persona Journal (Middle)
            if let Ok(ponders) = db.get_ponderings(20) {
                let items: Vec<ListItem> = ponders.iter().map(|p| {
                    ListItem::new(format!("> {}", p)).style(Style::default().fg(Color::Magenta))
                }).collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(" Persona Journal "));
                f.render_widget(list, body_chunks[1]);
            }

            // 3. Command Pulse (Right)
            if let Ok(execs) = db.get_recent_executions(24) {
                let items: Vec<ListItem> = execs.iter().map(|(cmd, args, status)| {
                    let style = if status == "success" {
                        Style::default().fg(Color::Green)
                    } else {
                        Style::default().fg(Color::Red)
                    };
                    ListItem::new(format!("[{}] {} {}", status.to_uppercase(), cmd, args)).style(style)
                }).collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(" Command Pulse "));
                f.render_widget(list, body_chunks[2]);
            }

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
