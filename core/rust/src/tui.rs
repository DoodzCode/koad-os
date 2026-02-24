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
    // 1. Setup Terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 2. Event Loop
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

            // Header: Status
            let header = Paragraph::new("KoadOS Real-Time Dashboard - [OBSERVER MODE]")
                .style(Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(header, chunks[0]);

            // Main Body: Split between Memories and Ponderings
            let body_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Percentage(50),
                    Constraint::Percentage(50),
                ])
                .split(chunks[1]);

            // Left: Knowledge Feed
            if let Ok(recent) = db.query("") {
                let items: Vec<ListItem> = recent.iter().take(15).map(|(_, cat, content, ts)| {
                    let style = match cat.as_str() {
                        "fact" => Style::default().fg(Color::Cyan),
                        "learning" => Style::default().fg(Color::Green),
                        _ => Style::default().fg(Color::White),
                    };
                    ListItem::new(format!("[{}] {} - {}", ts, cat, content)).style(style)
                }).collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(" Knowledge Stream "));
                f.render_widget(list, body_chunks[0]);
            }

            // Right: Pondering/Reflection Stream
            if let Ok(ponders) = db.get_ponderings(10) {
                let items: Vec<ListItem> = ponders.iter().map(|p| {
                    ListItem::new(format!("- {}", p)).style(Style::default().fg(Color::Magenta))
                }).collect();
                let list = List::new(items)
                    .block(Block::default().borders(Borders::ALL).title(" Persona Journal "));
                f.render_widget(list, body_chunks[1]);
            }

            // Footer: Keybindings
            let footer = Paragraph::new("Press 'q' to exit | Auto-refreshing every 500ms")
                .style(Style::default().fg(Color::DarkGray))
                .block(Block::default().borders(Borders::ALL));
            f.render_widget(footer, chunks[2]);
        })?;

        // 3. Handle Key Events
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

    // 4. Cleanup Terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())
}
