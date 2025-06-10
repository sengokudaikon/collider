use clap::{Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(name = "seed_database")]
#[command(about = "Database seeding tool with progress visualization")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long)]
    pub database_url: Option<String>,

    #[arg(short, long, default_value = "interactive")]
    pub mode: ProgressMode,
}

#[derive(ValueEnum, Clone, Debug)]
pub enum ProgressMode {
    /// No progress output, explicit args required  
    Quiet,
    /// Interactive prompts with progress bars (default)
    Interactive,
}

#[derive(Subcommand)]
pub enum Commands {
    All {
        #[arg(long)]
        min_users: Option<usize>,

        #[arg(long)]
        max_users: Option<usize>,

        #[arg(long)]
        min_event_types: Option<usize>,

        #[arg(long)]
        max_event_types: Option<usize>,

        #[arg(long)]
        target_events: Option<usize>,

        #[arg(long)]
        event_batch_size: Option<usize>,
    },

    Users {
        #[arg(long)]
        min_users: Option<usize>,

        #[arg(long)]
        max_users: Option<usize>,
    },

    EventTypes {
        #[arg(long)]
        min_types: Option<usize>,

        #[arg(long)]
        max_types: Option<usize>,
    },

    Events {
        #[arg(long)]
        target_events: Option<usize>,

        #[arg(long)]
        batch_size: Option<usize>,
    },
}

impl Cli {
    pub fn get_database_url(&self) -> String {
        self.database_url.clone().unwrap_or_else(|| {
            std::env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgresql://postgres:postgres@localhost:5432/postgres"
                    .to_string()
            })
        })
    }
}
