use std::time::Instant;

use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use database_traits::connection::GetDatabaseConnect;
use events_models::{event_types, events};
use rand::{Rng, prelude::IndexedRandom, rng};
use sea_orm::{EntityTrait, Set};
use serde_json::json;
use sql_connection::SqlConnect;
use tracing::{info, instrument, warn};
use user_models as users;
use uuid::Uuid;

use crate::{ProgressTracker, ProgressUpdate, Seeder};

pub struct EventSeeder {
    db: SqlConnect,
    target_events: usize,
    batch_size: usize,
}

impl EventSeeder {
    pub fn new(
        db: SqlConnect, target_events: usize, batch_size: usize,
    ) -> Self {
        Self {
            db,
            target_events,
            batch_size,
        }
    }

    #[instrument(skip(self))]
    async fn get_available_users(&self) -> Result<Vec<Uuid>> {
        let db = self.db.get_connect();
        let users_data = users::Entity::find().all(db).await?;

        if users_data.is_empty() {
            anyhow::bail!(
                "No users found in database. Run UserSeeder first."
            );
        }

        let user_ids: Vec<Uuid> =
            users_data.into_iter().map(|u| u.id).collect();
        info!("Found {} users for event generation", user_ids.len());
        Ok(user_ids)
    }

    #[instrument(skip(self))]
    async fn get_available_event_types(&self) -> Result<Vec<i32>> {
        let db = self.db.get_connect();
        let event_types_data = event_types::Entity::find().all(db).await?;

        if event_types_data.is_empty() {
            anyhow::bail!(
                "No event types found in database. Run EventTypeSeeder \
                 first."
            );
        }

        let event_type_ids: Vec<i32> =
            event_types_data.into_iter().map(|et| et.id).collect();
        info!(
            "Found {} event types for event generation",
            event_type_ids.len()
        );
        Ok(event_type_ids)
    }

    fn generate_metadata(&self) -> serde_json::Value {
        let mut rng = rng();

        match rng.random_range(0..5) {
            0 => {
                json!({
                    "page_url": format!("/page/{}", rng.random_range(1..1000)),
                    "referrer": "https://google.com",
                    "user_agent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7)"
                })
            }
            1 => {
                json!({
                    "button_id": format!("btn_{}", rng.random_range(1..100)),
                    "element_text": "Click me",
                    "coordinates": {"x": rng.random_range(0..1920), "y": rng.random_range(0..1080)}
                })
            }
            2 => {
                json!({
                    "form_id": format!("form_{}", rng.random_range(1..50)),
                    "field_count": rng.random_range(1..20),
                    "completion_time_ms": rng.random_range(1000..60000)
                })
            }
            3 => {
                json!({
                    "product_id": rng.random_range(1..10000),
                    "price": rng.random_range(10..1000) as f64 / 100.0,
                    "category": format!("category_{}", rng.random_range(1..20))
                })
            }
            _ => {
                json!({
                    "session_id": Uuid::now_v7(),
                    "duration_ms": rng.random_range(1000..300000),
                    "device_type": match rng.random_range(0..3) {
                        0 => "mobile",
                        1 => "desktop",
                        _ => "tablet"
                    }
                })
            }
        }
    }

    #[instrument(skip(self, user_ids, event_type_ids, progress_tracker), fields(batch_size = self.batch_size))]
    async fn generate_event_batch(
        &self, user_ids: &[Uuid], event_type_ids: &[i32], batch_start: usize,
        batch_size: usize, progress_tracker: &Option<ProgressTracker>,
    ) -> Result<()> {
        let db = self.db.get_connect();
        let mut batch_events = Vec::with_capacity(batch_size);

        let start_time = Utc::now() - Duration::days(30);
        let end_time = Utc::now();
        let time_range_seconds = (end_time - start_time).num_seconds();

        for _ in 0..batch_size {
            let (user_id, event_type_id, random_seconds) = {
                let mut rng = rng();
                let user_id = *user_ids.choose(&mut rng).unwrap();
                let event_type_id = *event_type_ids.choose(&mut rng).unwrap();
                let random_seconds = rng.random_range(0..time_range_seconds);
                (user_id, event_type_id, random_seconds)
            };

            let timestamp = start_time + Duration::seconds(random_seconds);

            let metadata = self.generate_metadata();

            let active_event = events::ActiveModel {
                id: Set(Uuid::now_v7()),
                user_id: Set(user_id),
                event_type_id: Set(event_type_id),
                timestamp: Set(timestamp),
                metadata: Set(Some(metadata)),
            };

            batch_events.push(active_event);
        }

        events::Entity::insert_many(batch_events).exec(db).await?;

        let current_total = batch_start + batch_size;

        // Send progress update if tracker is available
        if let Some(tracker) = progress_tracker {
            let progress_percentage =
                (current_total as f64 / self.target_events as f64) * 100.0;
            tracker.update(ProgressUpdate {
                seeder_name: "EventSeeder".to_string(),
                current: current_total,
                total: self.target_events,
                message: format!(
                    "Generated {} events ({:.1}% complete)",
                    current_total, progress_percentage
                ),
            });
        }

        if current_total % 100000 == 0 {
            info!(
                "Generated {} events ({:.1}% complete)",
                current_total,
                (current_total as f64 / self.target_events as f64) * 100.0
            );
        }

        Ok(())
    }

    #[instrument(skip(self, progress_tracker))]
    async fn generate_events(
        &self, user_ids: Vec<Uuid>, event_type_ids: Vec<i32>,
        progress_tracker: &Option<ProgressTracker>,
    ) -> Result<()> {
        let generation_start = Instant::now();
        info!(
            "Starting generation of {} events in batches of {}",
            self.target_events, self.batch_size
        );

        let total_batches = self.target_events.div_ceil(self.batch_size);
        let mut events_generated = 0;

        for batch_num in 0..total_batches {
            let batch_start = batch_num * self.batch_size;
            let remaining_events = self.target_events - batch_start;
            let current_batch_size =
                std::cmp::min(self.batch_size, remaining_events);

            if current_batch_size == 0 {
                break;
            }

            let batch_start_time = Instant::now();
            match self
                .generate_event_batch(
                    &user_ids,
                    &event_type_ids,
                    batch_start,
                    current_batch_size,
                    progress_tracker,
                )
                .await
            {
                Ok(_) => {
                    events_generated += current_batch_size;
                    let batch_time = batch_start_time.elapsed();
                    let events_per_sec =
                        current_batch_size as f64 / batch_time.as_secs_f64();

                    // Log progress every 10 batches or for large batches
                    if (batch_num + 1) % 10 == 0
                        || current_batch_size >= 10000
                    {
                        let elapsed = generation_start.elapsed();
                        let overall_rate =
                            events_generated as f64 / elapsed.as_secs_f64();
                        let progress = (events_generated as f64
                            / self.target_events as f64)
                            * 100.0;

                        info!(
                            "Batch {}/{}: {} events/sec (batch), {:.0} \
                             events/sec (overall), {:.1}% complete",
                            batch_num + 1,
                            total_batches,
                            events_per_sec as u32,
                            overall_rate,
                            progress
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Failed to generate batch {}: {}",
                        batch_num + 1,
                        e
                    );
                    return Err(e);
                }
            }
        }

        let total_time = generation_start.elapsed();
        let overall_rate =
            self.target_events as f64 / total_time.as_secs_f64();

        info!(
            "Successfully generated {} events in {:.2}s ({:.0} events/sec)",
            self.target_events,
            total_time.as_secs_f64(),
            overall_rate
        );
        Ok(())
    }
}

#[async_trait]
impl Seeder for EventSeeder {
    async fn seed(&self) -> Result<()> {
        info!("Seeding {} events", self.target_events);

        let user_ids = self.get_available_users().await?;
        let event_type_ids = self.get_available_event_types().await?;

        self.generate_events(user_ids, event_type_ids, &None)
            .await?;

        info!("Successfully seeded {} events", self.target_events);
        Ok(())
    }

    async fn seed_with_progress(
        &self, progress_tracker: Option<ProgressTracker>,
    ) -> Result<()> {
        info!(
            "Seeding {} events with progress tracking",
            self.target_events
        );

        // Send initial progress update
        if let Some(ref tracker) = progress_tracker {
            tracker.update(ProgressUpdate {
                seeder_name: "EventSeeder".to_string(),
                current: 0,
                total: self.target_events,
                message: "Starting event generation...".to_string(),
            });
        }

        let user_ids = self.get_available_users().await?;
        let event_type_ids = self.get_available_event_types().await?;

        self.generate_events(user_ids, event_type_ids, &progress_tracker)
            .await?;

        // Send completion update
        if let Some(ref tracker) = progress_tracker {
            tracker.update(ProgressUpdate {
                seeder_name: "EventSeeder".to_string(),
                current: self.target_events,
                total: self.target_events,
                message: "Event generation complete".to_string(),
            });
        }

        info!("Successfully seeded {} events", self.target_events);
        Ok(())
    }

    fn name(&self) -> &'static str { "EventSeeder" }
}
