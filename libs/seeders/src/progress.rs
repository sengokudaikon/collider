use std::{
    io::{self, Stdout},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode},
    execute,
    terminal::{
        EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
        enable_raw_mode,
    },
};
use ratatui::{
    Frame, Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Gauge, Paragraph},
};
use tokio::sync::mpsc;

#[derive(Debug, Clone)]
pub struct ProgressUpdate {
    pub seeder_name: String,
    pub current: usize,
    pub total: usize,
    pub message: String,
}

#[derive(Debug, Clone)]
pub enum ProgressEvent {
    Update(ProgressUpdate),
    Complete(String),
    Error(String, String),
    Finish,
}

#[derive(Clone)]
pub struct ProgressTracker {
    tx: mpsc::UnboundedSender<ProgressEvent>,
}

impl ProgressTracker {
    pub fn new() -> (Self, mpsc::UnboundedReceiver<ProgressEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { tx }, rx)
    }

    pub fn update(&self, update: ProgressUpdate) {
        let _ = self.tx.send(ProgressEvent::Update(update));
    }

    pub fn complete(&self, seeder_name: String) {
        let _ = self.tx.send(ProgressEvent::Complete(seeder_name));
    }

    pub fn error(&self, seeder_name: String, error: String) {
        let _ = self.tx.send(ProgressEvent::Error(seeder_name, error));
    }

    pub fn finish(&self) { let _ = self.tx.send(ProgressEvent::Finish); }
}

#[derive(Debug, Clone)]
struct SeederProgress {
    name: String,
    current: usize,
    total: usize,
    message: String,
    completed: bool,
    error: Option<String>,
    start_time: Instant,
}

impl SeederProgress {
    fn new(name: String) -> Self {
        Self {
            name,
            current: 0,
            total: 0,
            message: "Starting...".to_string(),
            completed: false,
            error: None,
            start_time: Instant::now(),
        }
    }

    fn progress_ratio(&self) -> f64 {
        if self.total == 0 {
            0.0
        }
        else {
            self.current as f64 / self.total as f64
        }
    }

    fn elapsed(&self) -> Duration { self.start_time.elapsed() }

    fn estimated_remaining(&self) -> Option<Duration> {
        if self.current == 0 || self.completed {
            None
        }
        else {
            let elapsed = self.elapsed();
            let rate = self.current as f64 / elapsed.as_secs_f64();
            let remaining = (self.total - self.current) as f64 / rate;
            Some(Duration::from_secs_f64(remaining))
        }
    }
}

pub struct ProgressUI {
    terminal: Terminal<CrosstermBackend<Stdout>>,
    seeders: Arc<Mutex<Vec<SeederProgress>>>,
    start_time: Instant,
}

impl ProgressUI {
    pub fn new() -> Result<Self> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            terminal,
            seeders: Arc::new(Mutex::new(Vec::new())),
            start_time: Instant::now(),
        })
    }

    pub async fn run(
        &mut self, mut rx: mpsc::UnboundedReceiver<ProgressEvent>,
    ) -> Result<()> {
        loop {
            tokio::select! {
                Some(event) = rx.recv() => {
                    match event {
                        ProgressEvent::Update(update) => {
                            self.update_seeder(update);
                        }
                        ProgressEvent::Complete(name) => {
                            self.complete_seeder(name);
                        }
                        ProgressEvent::Error(name, error) => {
                            self.error_seeder(name, error);
                        }
                        ProgressEvent::Finish => {
                            break;
                        }
                    }
                }
                _ = tokio::time::sleep(Duration::from_millis(100)) => {}
            }

            let elapsed_secs = self.start_time.elapsed().as_secs_f64();
            let seeders = self.seeders.lock().unwrap().to_vec();
            self.terminal.draw(|f| {
                Self::render_ui(f, elapsed_secs, &seeders);
            })?;

            if event::poll(Duration::from_millis(0))? {
                if let Event::Key(key) = event::read()? {
                    if key.code == KeyCode::Char('q')
                        || key.code == KeyCode::Esc
                    {
                        break;
                    }
                }
            }
        }

        Ok(())
    }

    fn update_seeder(&self, update: ProgressUpdate) {
        let mut seeders = self.seeders.lock().unwrap();

        if let Some(seeder) =
            seeders.iter_mut().find(|s| s.name == update.seeder_name)
        {
            seeder.current = update.current;
            seeder.total = update.total;
            seeder.message = update.message;
        }
        else {
            let mut new_seeder = SeederProgress::new(update.seeder_name);
            new_seeder.current = update.current;
            new_seeder.total = update.total;
            new_seeder.message = update.message;
            seeders.push(new_seeder);
        }
    }

    fn complete_seeder(&self, name: String) {
        let mut seeders = self.seeders.lock().unwrap();
        if let Some(seeder) = seeders.iter_mut().find(|s| s.name == name) {
            seeder.completed = true;
            seeder.message = "Completed!".to_string();
            seeder.current = seeder.total;
        }
    }

    fn error_seeder(&self, name: String, error: String) {
        let mut seeders = self.seeders.lock().unwrap();
        if let Some(seeder) = seeders.iter_mut().find(|s| s.name == name) {
            seeder.error = Some(error.clone());
            seeder.message = format!("Error: {}", error);
        }
    }

    fn render_ui(
        f: &mut Frame, elapsed_secs: f64, seeders: &[SeederProgress],
    ) {
        let size = f.size();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(3),
            ])
            .split(size);

        let header = Paragraph::new(vec![
            Line::from(vec![Span::styled(
                "Database Seeder Progress",
                Style::default().add_modifier(Modifier::BOLD),
            )]),
            Line::from(vec![Span::raw(format!(
                "Total elapsed: {:.1}s",
                elapsed_secs
            ))]),
        ])
        .block(Block::default().borders(Borders::ALL).title("Status"));
        f.render_widget(header, chunks[0]);

        Self::render_progress_bars_static(f, chunks[1], seeders);

        let footer = Paragraph::new("Press 'q' or 'Esc' to quit")
            .style(Style::default().fg(Color::Gray))
            .block(Block::default().borders(Borders::ALL));
        f.render_widget(footer, chunks[2]);
    }

    fn render_progress_bars_static(
        f: &mut Frame, area: Rect, seeders: &[SeederProgress],
    ) {
        if seeders.is_empty() {
            let empty = Paragraph::new("Waiting for seeders to start...")
                .block(
                    Block::default().borders(Borders::ALL).title("Seeders"),
                );
            f.render_widget(empty, area);
            return;
        }

        let constraints: Vec<Constraint> =
            seeders.iter().map(|_| Constraint::Length(4)).collect();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        for (i, seeder) in seeders.iter().enumerate() {
            if i >= chunks.len() {
                break;
            }

            Self::render_seeder_progress_static(f, chunks[i], seeder);
        }
    }

    fn render_seeder_progress_static(
        f: &mut Frame, area: Rect, seeder: &SeederProgress,
    ) {
        let progress_ratio = seeder.progress_ratio();

        let (color, status_text) = if seeder.error.is_some() {
            (Color::Red, "ERROR")
        }
        else if seeder.completed {
            (Color::Green, "COMPLETE")
        }
        else {
            (Color::Blue, "RUNNING")
        };

        let title = format!("{} [{}]", seeder.name, status_text);

        let mut label = format!(
            "{}/{} ({}%) - {}",
            seeder.current,
            seeder.total,
            (progress_ratio * 100.0) as u32,
            seeder.message
        );

        if let Some(remaining) = seeder.estimated_remaining() {
            label.push_str(&format!(
                " - ETA: {:.1}s",
                remaining.as_secs_f64()
            ));
        }

        let gauge = Gauge::default()
            .block(Block::default().borders(Borders::ALL).title(title))
            .gauge_style(Style::default().fg(color))
            .ratio(progress_ratio)
            .label(label);

        f.render_widget(gauge, area);
    }
}

impl Drop for ProgressUI {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
    }
}
