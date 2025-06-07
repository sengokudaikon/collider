use anyhow::Result;
use sql_connection::SqlConnect;
use tracing::{info, instrument, warn};

use crate::Seeder;

pub struct SeederRunner {
    db: SqlConnect,
    seeders: Vec<Box<dyn Seeder>>,
}

impl SeederRunner {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            db,
            seeders: Vec::new(),
        }
    }

    pub fn add_seeder(mut self, seeder: Box<dyn Seeder>) -> Self {
        self.seeders.push(seeder);
        self
    }

    #[instrument(skip(self))]
    pub async fn run_all(&self) -> Result<()> {
        info!(
            "Starting seeding process for {} seeders",
            self.seeders.len()
        );

        for seeder in &self.seeders {
            info!("Running seeder: {}", seeder.name());
            match seeder.seed().await {
                Ok(_) => {
                    info!(
                        "✓ Seeder '{}' completed successfully",
                        seeder.name()
                    )
                }
                Err(e) => {
                    warn!("✗ Seeder '{}' failed: {}", seeder.name(), e);
                    return Err(e);
                }
            }
        }

        info!("All seeders completed successfully");
        Ok(())
    }

    pub fn get_connection(&self) -> &SqlConnect { &self.db }
}
