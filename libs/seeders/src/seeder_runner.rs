use std::time::Instant;

use anyhow::Result;
use futures::future;
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

    #[instrument(skip(self))]
    pub async fn run_parallel(&self) -> Result<()> {
        info!(
            "Starting parallel seeding process for {} seeders",
            self.seeders.len()
        );

        // Separate seeders by dependency
        let mut user_seeders = Vec::new();
        let mut event_type_seeders = Vec::new();
        let mut event_seeders = Vec::new();
        let mut other_seeders = Vec::new();

        for seeder in &self.seeders {
            match seeder.name() {
                "UserSeeder" => user_seeders.push(seeder),
                "EventTypeSeeder" => event_type_seeders.push(seeder),
                "EventSeeder" => event_seeders.push(seeder),
                _ => other_seeders.push(seeder),
            }
        }

        // Phase 1: Run User and EventType seeders in parallel (they're
        // independent)
        if !user_seeders.is_empty() || !event_type_seeders.is_empty() {
            info!("Phase 1: Running User and EventType seeders in parallel");

            let phase1_seeders: Vec<_> = user_seeders
                .iter()
                .chain(event_type_seeders.iter())
                .collect();
            let mut phase1_futures = Vec::new();

            for &seeder in &phase1_seeders {
                info!("Starting parallel seeder: {}", seeder.name());
                let progress_tracker = self.progress_tracker.clone();
                let seeder_name = seeder.name();

                let future = async move {
                    let seeder_start = Instant::now();
                    let result =
                        seeder.seed_with_progress(progress_tracker).await;
                    let seeder_time = seeder_start.elapsed();
                    (seeder_name, result, seeder_time)
                };

                phase1_futures.push(future);
            }

            // Wait for all phase 1 seeders to complete
            let phase1_results = future::join_all(phase1_futures).await;

            for (seeder_name, result, seeder_time) in phase1_results {
                match result {
                    Ok(_) => {
                        info!(
                            "Parallel seeder '{}' completed successfully in \
                             {:.2}s",
                            seeder_name,
                            seeder_time.as_secs_f64()
                        );

                        if let Some(tracker) = &self.progress_tracker {
                            tracker.complete(seeder_name.to_string());
                        }
                    }
                    Err(e) => {
                        warn!(
                            "Parallel seeder '{}' failed after {:.2}s: {}",
                            seeder_name,
                            seeder_time.as_secs_f64(),
                            e
                        );

                        if let Some(tracker) = &self.progress_tracker {
                            tracker.error(
                                seeder_name.to_string(),
                                e.to_string(),
                            );
                        }

                        return Err(e);
                    }
                }
            }
        }

        // Phase 2: Run Event seeders (they depend on Users and EventTypes)
        for seeder in event_seeders.iter().chain(other_seeders.iter()) {
            let seeder_start = Instant::now();
            info!("Running dependent seeder: {}", seeder.name());

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

        info!("All parallel seeders completed successfully");
        info!("Performance improvements applied:");
        info!("  ✓ Parallel User/EventType seeding");
        info!(
            "  ✓ Parallel batch processing in EventSeeder (8 concurrent \
             batches)"
        );
        info!("  ✓ Optimized connection pool (100 max connections)");
        info!("  ✓ Reduced SQL query logging for bulk operations");
        Ok(())
    }

    pub fn get_connection(&self) -> &SqlConnect { &self.db }
}
