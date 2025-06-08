pub mod cli;
pub mod event_seeder;
pub mod event_type_seeder;
pub mod progress;
pub mod seeder_runner;
pub mod user_seeder;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Seeder: Send + Sync {
    async fn seed(&self) -> Result<()>;
    fn name(&self) -> &'static str;
}

pub use cli::{Cli, Commands};
pub use event_seeder::EventSeeder;
pub use event_type_seeder::EventTypeSeeder;
pub use progress::{ProgressTracker, ProgressUI, ProgressUpdate, ProgressEvent};
pub use seeder_runner::SeederRunner;
pub use user_seeder::UserSeeder;
