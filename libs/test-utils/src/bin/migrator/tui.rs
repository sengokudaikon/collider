use std::io;

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
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap,
    },
};
use test_utils::SqlMigrator;

struct App {
    migrator: SqlMigrator,
    selected_index: usize,
    menu_items: Vec<MenuItem>,
    status_text: String,
    applied_migrations: Vec<String>,
    show_confirmation: bool,
    confirmation_action: ConfirmationAction,
}

#[derive(Clone)]
struct MenuItem {
    title: String,
    description: String,
    action: Action,
}

#[derive(Clone)]
enum Action {
    RunUp,
    RunDown,
    Reset,
    Status,
    Quit,
}

#[derive(Clone)]
enum ConfirmationAction {
    None,
    Reset,
    RunDown,
}

impl App {
    async fn new(migrator: SqlMigrator) -> Result<Self> {
        let applied_migrations =
            migrator.list_applied_migrations().await.unwrap_or_default();

        Ok(App {
            migrator,
            selected_index: 0,
            menu_items: vec![
                MenuItem {
                    title: "Run Migrations (Up)".to_string(),
                    description: "Apply all pending migrations".to_string(),
                    action: Action::RunUp,
                },
                MenuItem {
                    title: "Rollback Migrations (Down)".to_string(),
                    description: "Roll back the last migration".to_string(),
                    action: Action::RunDown,
                },
                MenuItem {
                    title: "Reset All Migrations".to_string(),
                    description: "⚠️  WARNING: Delete all data and reset \
                                  migrations"
                        .to_string(),
                    action: Action::Reset,
                },
                MenuItem {
                    title: "Refresh Status".to_string(),
                    description: "Reload migration status".to_string(),
                    action: Action::Status,
                },
                MenuItem {
                    title: "Quit".to_string(),
                    description: "Exit the migration tool".to_string(),
                    action: Action::Quit,
                },
            ],
            status_text: format!(
                "Ready. {} migrations applied.",
                applied_migrations.len()
            ),
            applied_migrations,
            show_confirmation: false,
            confirmation_action: ConfirmationAction::None,
        })
    }

    fn next(&mut self) {
        self.selected_index =
            (self.selected_index + 1) % self.menu_items.len();
    }

    fn previous(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
        }
        else {
            self.selected_index = self.menu_items.len() - 1;
        }
    }

    async fn execute_action(&mut self, action: Action) -> Result<()> {
        match action {
            Action::RunUp => {
                self.status_text = "Running migrations...".to_string();
                match self.migrator.run_all_migrations().await {
                    Ok(_) => {
                        self.applied_migrations = self
                            .migrator
                            .list_applied_migrations()
                            .await
                            .unwrap_or_default();
                        self.status_text = format!(
                            "✓ Migrations completed. {} applied.",
                            self.applied_migrations.len()
                        );
                    }
                    Err(e) => {
                        self.status_text =
                            format!("❌ Migration failed: {}", e);
                    }
                }
            }
            Action::RunDown => {
                if self.applied_migrations.is_empty() {
                    self.status_text =
                        "No migrations to roll back".to_string();
                    return Ok(());
                }

                self.show_confirmation = true;
                self.confirmation_action = ConfirmationAction::RunDown;
            }
            Action::Reset => {
                self.show_confirmation = true;
                self.confirmation_action = ConfirmationAction::Reset;
            }
            Action::Status => {
                self.applied_migrations = self
                    .migrator
                    .list_applied_migrations()
                    .await
                    .unwrap_or_default();
                self.status_text = format!(
                    "Status refreshed. {} migrations applied.",
                    self.applied_migrations.len()
                );
            }
            Action::Quit => {}
        }
        Ok(())
    }

    async fn confirm_action(&mut self) -> Result<bool> {
        match self.confirmation_action.clone() {
            ConfirmationAction::Reset => {
                self.status_text = "Resetting all migrations...".to_string();
                match self.migrator.reset_all().await {
                    Ok(_) => {
                        self.applied_migrations.clear();
                        self.status_text =
                            "✓ All migrations reset successfully".to_string();
                    }
                    Err(e) => {
                        self.status_text = format!("❌ Reset failed: {}", e);
                    }
                }
            }
            ConfirmationAction::RunDown => {
                if let Some(last_migration) = self.applied_migrations.last() {
                    self.status_text =
                        format!("Rolling back migration: {}", last_migration);
                    let to_rollback = vec![last_migration.as_str()];
                    match self
                        .migrator
                        .run_down_migrations(&to_rollback)
                        .await
                    {
                        Ok(_) => {
                            self.applied_migrations = self
                                .migrator
                                .list_applied_migrations()
                                .await
                                .unwrap_or_default();
                            self.status_text = format!(
                                "✓ Rollback completed. {} migrations remain.",
                                self.applied_migrations.len()
                            );
                        }
                        Err(e) => {
                            self.status_text =
                                format!("❌ Rollback failed: {}", e);
                        }
                    }
                }
            }
            ConfirmationAction::None => {}
        }

        self.show_confirmation = false;
        self.confirmation_action = ConfirmationAction::None;
        Ok(false)
    }

    fn cancel_confirmation(&mut self) {
        self.show_confirmation = false;
        self.confirmation_action = ConfirmationAction::None;
        self.status_text = "Action cancelled".to_string();
    }
}

pub async fn run_tui(migrator: SqlMigrator) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = App::new(migrator).await?;
    let res = run_app(&mut terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{:?}", err);
    }

    Ok(())
}

async fn run_app<B: Backend>(
    terminal: &mut Terminal<B>, app: &mut App,
) -> Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if let Event::Key(key) = event::read()? {
            if app.show_confirmation {
                match key.code {
                    KeyCode::Char('y')
                    | KeyCode::Char('Y')
                    | KeyCode::Enter => {
                        if app.confirm_action().await? {
                            return Ok(());
                        }
                    }
                    KeyCode::Char('n')
                    | KeyCode::Char('N')
                    | KeyCode::Esc => {
                        app.cancel_confirmation();
                    }
                    _ => {}
                }
            }
            else {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Up | KeyCode::Char('k') => app.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.next(),
                    KeyCode::Enter | KeyCode::Char(' ') => {
                        let action =
                            app.menu_items[app.selected_index].action.clone();
                        if matches!(action, Action::Quit) {
                            return Ok(());
                        }
                        app.execute_action(action).await?;
                    }
                    _ => {}
                }
            }
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(8),
            Constraint::Length(3),
        ])
        .split(f.area());

    let title = Paragraph::new("Database Migration Tool")
        .style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
        .block(Block::default().borders(Borders::ALL));
    f.render_widget(title, chunks[0]);

    let menu_items: Vec<ListItem> = app
        .menu_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let style = if i == app.selected_index {
                Style::default().fg(Color::Black).bg(Color::White)
            }
            else {
                Style::default().fg(Color::White)
            };

            ListItem::new(vec![
                Line::from(vec![Span::styled(
                    &item.title,
                    style.add_modifier(Modifier::BOLD),
                )]),
                Line::from(vec![Span::styled(
                    format!("  {}", &item.description),
                    style,
                )]),
            ])
        })
        .collect();

    let menu = List::new(menu_items)
        .block(Block::default().borders(Borders::ALL).title("Actions"))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    let mut list_state = ListState::default();
    list_state.select(Some(app.selected_index));
    f.render_stateful_widget(menu, chunks[1], &mut list_state);

    let migration_items: Vec<ListItem> = if app.applied_migrations.is_empty()
    {
        vec![ListItem::new("No migrations applied")]
    }
    else {
        app.applied_migrations
            .iter()
            .map(|migration| {
                ListItem::new(format!("✓ {}", migration))
                    .style(Style::default().fg(Color::Green))
            })
            .collect()
    };

    let migrations = List::new(migration_items).block(
        Block::default()
            .borders(Borders::ALL)
            .title("Applied Migrations"),
    );
    f.render_widget(migrations, chunks[2]);

    let status = Paragraph::new(app.status_text.as_str())
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title("Status"));
    f.render_widget(status, chunks[3]);

    if app.show_confirmation {
        let area = chunks[1];
        let popup_area = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage(50),
                Constraint::Min(7),
                Constraint::Percentage(50),
            ])
            .split(area)[1];

        let popup_area = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(20),
                Constraint::Min(40),
                Constraint::Percentage(20),
            ])
            .split(popup_area)[1];

        f.render_widget(Clear, popup_area);

        let confirmation_text = match app.confirmation_action {
            ConfirmationAction::Reset => {
                "⚠️  WARNING: This will DELETE ALL DATA!\n\nAre you sure you \
                 want to reset all migrations?\n\n[Y]es / [N]o"
            }
            ConfirmationAction::RunDown => {
                "Roll back the last migration?\n\n[Y]es / [N]o"
            }
            ConfirmationAction::None => "",
        };

        let confirmation = Paragraph::new(confirmation_text)
            .wrap(Wrap { trim: true })
            .style(Style::default().fg(Color::Yellow))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Confirmation")
                    .style(Style::default().fg(Color::Red)),
            );
        f.render_widget(confirmation, popup_area);
    }
}
