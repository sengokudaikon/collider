use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
#[command(name = "seed_database")]
#[command(about = "Database seeding tool with progress visualization")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, env = "DATABASE_URL")]
    pub database_url: Option<String>,

    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    All {
        #[arg(long, default_value = "10000")]
        min_users: usize,

        #[arg(long, default_value = "100000")]
        max_users: usize,

        #[arg(long, default_value = "50")]
        min_event_types: usize,

        #[arg(long, default_value = "200")]
        max_event_types: usize,

        #[arg(long, default_value = "10000000")]
        target_events: usize,

        #[arg(long)]
        event_batch_size: Option<usize>,
    },

    Users {
        #[arg(long, default_value = "10000")]
        min_users: usize,

        #[arg(long, default_value = "100000")]
        max_users: usize,
    },

    EventTypes {
        #[arg(long, default_value = "50")]
        min_types: usize,

        #[arg(long, default_value = "200")]
        max_types: usize,
    },

    Events {
        #[arg(long, default_value = "10000000")]
        target_events: usize,

        #[arg(long)]
        batch_size: Option<usize>,
    },
}

impl Cli {
    pub fn get_database_url(&self) -> String {
        self.database_url.clone().unwrap_or_else(|| {
            "postgresql://user:password@localhost:5432/collider".to_string()
        })
    }
}
