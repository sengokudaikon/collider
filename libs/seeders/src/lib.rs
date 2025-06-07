pub mod event_seeder;
pub mod event_type_seeder;
pub mod seeder_runner;
pub mod user_seeder;

use anyhow::Result;
use async_trait::async_trait;

#[async_trait]
pub trait Seeder: Send + Sync {
    async fn seed(&self) -> Result<()>;
    fn name(&self) -> &'static str;
}

pub use event_seeder::EventSeeder;
pub use event_type_seeder::EventTypeSeeder;
pub use seeder_runner::SeederRunner;
pub use user_seeder::UserSeeder;
