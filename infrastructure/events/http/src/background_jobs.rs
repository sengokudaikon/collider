use std::time::Duration;

use sql_connection::SqlConnect;
use tokio::time::interval;
use tracing::{error, info, warn};

#[derive(Clone)]
pub struct BackgroundJobScheduler {
    db: SqlConnect,
}

impl BackgroundJobScheduler {
    pub fn new(db: SqlConnect) -> Self { Self { db } }

    /// Start background job to refresh materialized views
    pub fn start_stats_refresh_job(self) {
        tokio::spawn(async move {
            let mut refresh_interval = interval(Duration::from_secs(900)); // 15 minutes
            refresh_interval.tick().await; // Skip first immediate tick

            info!(
                "Starting stats materialized view refresh job (every 15 \
                 minutes)"
            );

            loop {
                refresh_interval.tick().await;

                match self.refresh_stats_materialized_view().await {
                    Ok(_) => {
                        info!(
                            "Successfully refreshed stats materialized view"
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to refresh stats materialized view: {}",
                            e
                        );
                    }
                }
            }
        });
    }

    async fn refresh_stats_materialized_view(&self) -> anyhow::Result<()> {
        let client = self.db.get_client().await.map_err(|e| {
            anyhow::anyhow!("Database connection error: {}", e)
        })?;

        // Refresh materialized view
        let start = std::time::Instant::now();

        client
            .execute("REFRESH MATERIALIZED VIEW stats_summary", &[])
            .await
            .map_err(|e| {
                // If concurrent refresh fails (e.g., no unique index), try
                // regular refresh
                warn!(
                    "Concurrent refresh failed: {}, attempting regular \
                     refresh",
                    e
                );
                e
            })?;

        let duration = start.elapsed();
        info!("Materialized view refresh completed in {:?}", duration);

        Ok(())
    }

    /// Manually trigger stats refresh (for testing or on-demand refresh)
    pub async fn trigger_stats_refresh(&self) -> anyhow::Result<()> {
        self.refresh_stats_materialized_view().await
    }

    /// Alias for trigger_stats_refresh for compatibility
    pub async fn refresh_stats_now(&self) -> anyhow::Result<()> {
        self.refresh_stats_materialized_view().await
    }

    /// Start all background jobs
    pub async fn start(&self) {
        let scheduler = self.clone();
        scheduler.start_stats_refresh_job();
    }
}
