pub mod event_seeder;
pub mod event_type_seeder;
pub mod progress;
pub mod seeder_runner;
pub mod user_seeder;

use anyhow::Result;
use async_trait::async_trait;
use tokio_util::sync::CancellationToken;

#[async_trait]
pub trait Seeder: Send + Sync {
    async fn seed(&self) -> Result<()>;
    async fn seed_with_progress(
        &self, _progress_tracker: Option<ProgressTracker>,
    ) -> Result<()> {
        // Default implementation ignores progress tracking for backward
        // compatibility
        self.seed().await
    }
    async fn seed_with_cancellation(
        &self, _cancellation_token: CancellationToken,
    ) -> Result<()> {
        // Default implementation ignores cancellation for backward
        // compatibility
        self.seed().await
    }
    async fn seed_with_progress_and_cancellation(
        &self, progress_tracker: Option<ProgressTracker>,
        _cancellation_token: CancellationToken,
    ) -> Result<()> {
        // Default implementation combines both methods
        self.seed_with_progress(progress_tracker).await
    }
    fn name(&self) -> &'static str;
}

pub use event_seeder::EventSeeder;
pub use event_type_seeder::EventTypeSeeder;
pub use progress::{ProgressEvent, ProgressTracker, ProgressUpdate};
pub use seeder_runner::SeederRunner;
pub use user_seeder::UserSeeder;
