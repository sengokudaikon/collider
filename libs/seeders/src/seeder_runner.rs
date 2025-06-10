use std::time::Instant;

use anyhow::Result;
use sql_connection::SqlConnect;
use tracing::{info, instrument, warn};

use crate::{ProgressTracker, Seeder};

pub struct SeederRunner {
    db: SqlConnect,
    seeders: Vec<Box<dyn Seeder>>,
    progress_tracker: Option<ProgressTracker>,
}

impl SeederRunner {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            db,
            seeders: Vec::new(),
            progress_tracker: None,
        }
    }

    pub fn add_seeder(mut self, seeder: Box<dyn Seeder>) -> Self {
        self.seeders.push(seeder);
        self
    }

    pub fn with_progress(
        mut self, progress_tracker: ProgressTracker,
    ) -> Self {
        self.progress_tracker = Some(progress_tracker);
        self
    }

    #[instrument(skip(self))]
    pub async fn run_all(&self) -> Result<()> {
        info!(
            "Starting seeding process for {} seeders",
            self.seeders.len()
        );

        for seeder in &self.seeders {
            let seeder_start = Instant::now();
            info!("Running seeder: {}", seeder.name());

            match seeder
                .seed_with_progress(self.progress_tracker.clone())
                .await
            {
                Ok(_) => {
                    let seeder_time = seeder_start.elapsed();
                    info!(
                        "Seeder '{}' completed successfully in {:.2}s",
                        seeder.name(),
                        seeder_time.as_secs_f64()
                    );

                    if let Some(tracker) = &self.progress_tracker {
                        tracker.complete(seeder.name().to_string());
                    }
                }
                Err(e) => {
                    let seeder_time = seeder_start.elapsed();
                    warn!(
                        "Seeder '{}' failed after {:.2}s: {}",
                        seeder.name(),
                        seeder_time.as_secs_f64(),
                        e
                    );

                    if let Some(tracker) = &self.progress_tracker {
                        tracker
                            .error(seeder.name().to_string(), e.to_string());
                    }

                    return Err(e);
                }
            }
        }

        info!("All seeders completed successfully");
        Ok(())
    }

    pub fn get_connection(&self) -> &SqlConnect { &self.db }
}
