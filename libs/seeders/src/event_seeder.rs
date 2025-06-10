use std::{sync::Arc, time::Instant};

use anyhow::Result;
use async_trait::async_trait;
use chrono::{Duration, Utc};
use database_traits::connection::GetDatabaseConnect;
use events_models::{event_types, events};
use futures::{StreamExt, stream};
use rand::{Rng, prelude::IndexedRandom, rng};
use sea_orm::{EntityTrait, Set};
use serde_json::json;
use sql_connection::SqlConnect;
use tokio::sync::Semaphore;
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

    // Static version of metadata generation
    fn generate_metadata_static() -> serde_json::Value {
        use rand::{Rng, rng};

        let mut rng = rng();
        let variant = rng.random_range(0..5);

        match variant {
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

    #[instrument(skip(self, progress_tracker))]
    async fn generate_events_parallel(
        &self, user_ids: Vec<Uuid>, event_type_ids: Vec<i32>,
        progress_tracker: &Option<ProgressTracker>,
    ) -> Result<()> {
        let generation_start = Instant::now();
        info!(
            "Starting optimized parallel generation of {} events \
             (pre-generation + concurrent inserts)",
            self.target_events
        );

        // Phase 1: Pre-generate ALL event data in memory (CPU intensive)
        info!(
            "Phase 1: Pre-generating {} events in memory...",
            self.target_events
        );
        let pre_gen_start = Instant::now();

        let all_events = Self::pre_generate_events_bulk(
            self.target_events,
            &user_ids,
            &event_type_ids,
        )?;

        let pre_gen_time = pre_gen_start.elapsed();
        info!(
            "Pre-generation completed in {:.2}s ({:.0} events/sec)",
            pre_gen_time.as_secs_f64(),
            self.target_events as f64 / pre_gen_time.as_secs_f64()
        );

        // Phase 2: Split into chunks and distribute to workers (I/O
        // intensive)
        info!(
            "Phase 2: Distributing to {} concurrent database workers...",
            12
        );
        let chunks: Vec<Vec<events::ActiveModel>> = all_events
            .chunks(self.batch_size)
            .map(|chunk| chunk.to_vec())
            .collect();

        let total_chunks = chunks.len();
        let semaphore = Arc::new(Semaphore::new(12)); // More workers since no CPU work

        let results: Vec<Result<(usize, usize, f64)>> =
            stream::iter(chunks.into_iter().enumerate())
                .map(|(chunk_idx, chunk)| {
                    let semaphore = semaphore.clone();
                    let db = self.db.clone();
                    let progress_tracker = progress_tracker.clone();
                    let target_events = self.target_events;

                    async move {
                        let _permit =
                            semaphore.acquire().await.map_err(|e| {
                                anyhow::anyhow!("Semaphore error: {}", e)
                            })?;

                        let insert_start = Instant::now();
                        let chunk_size = chunk.len();

                        // Pure database insert - no data generation
                        Self::insert_pre_generated_batch(
                            &db,
                            chunk,
                            chunk_idx * self.batch_size,
                            target_events,
                            &progress_tracker,
                        )
                        .await?;

                        let insert_time = insert_start.elapsed();
                        let events_per_sec: f64 =
                            chunk_size as f64 / insert_time.as_secs_f64();

                        Ok((chunk_idx, chunk_size, events_per_sec))
                    }
                })
                .buffer_unordered(12) // 12 concurrent database workers
                .collect()
                .await;

        // Process results and calculate metrics
        let mut total_events_inserted = 0;
        let mut failed_chunks = 0;
        let mut max_throughput: f64 = 0.0;

        for result in results {
            match result {
                Ok((chunk_idx, chunk_size, events_per_sec)) => {
                    total_events_inserted += chunk_size;
                    max_throughput = max_throughput.max(events_per_sec);

                    // Log progress every 10 chunks
                    if (chunk_idx + 1) % 10 == 0 || chunk_size >= 10000 {
                        let elapsed = generation_start.elapsed();
                        let overall_rate = total_events_inserted as f64
                            / elapsed.as_secs_f64();
                        let progress = (total_events_inserted as f64
                            / self.target_events as f64)
                            * 100.0;

                        info!(
                            "Chunk {}/{}: {:.0} events/sec (chunk), {:.0} \
                             events/sec (overall), {:.1}% complete",
                            chunk_idx + 1,
                            total_chunks,
                            events_per_sec,
                            overall_rate,
                            progress
                        );
                    }
                }
                Err(e) => {
                    failed_chunks += 1;
                    warn!("Failed to insert chunk: {}", e);
                }
            }
        }

        if failed_chunks > 0 {
            return Err(anyhow::anyhow!(
                "{} chunks failed to insert",
                failed_chunks
            ));
        }

        let total_time = generation_start.elapsed();
        let overall_rate =
            self.target_events as f64 / total_time.as_secs_f64();

        info!("🚀 Optimized parallel seeding completed!");
        info!(
            "✓ Total: {} events in {:.2}s ({:.0} events/sec overall)",
            self.target_events,
            total_time.as_secs_f64(),
            overall_rate
        );
        info!(
            "✓ Pre-generation: {:.2}s, Concurrent inserts: {:.2}s",
            pre_gen_time.as_secs_f64(),
            (total_time - pre_gen_time).as_secs_f64()
        );
        info!("✓ Peak insert throughput: {:.0} events/sec", max_throughput);

        Ok(())
    }

    // Phase 1: Pre-generate ALL events in memory (CPU intensive,
    // single-threaded)
    fn pre_generate_events_bulk(
        target_events: usize, user_ids: &[Uuid], event_type_ids: &[i32],
    ) -> Result<Vec<events::ActiveModel>> {
        let mut all_events = Vec::with_capacity(target_events);
        let mut rng = rng();

        let start_time = Utc::now() - Duration::days(30);
        let end_time = Utc::now();
        let time_range_seconds = (end_time - start_time).num_seconds();

        // Bulk generate all events at once
        for _ in 0..target_events {
            let user_id = *user_ids.choose(&mut rng).unwrap();
            let event_type_id = *event_type_ids.choose(&mut rng).unwrap();
            let random_seconds = rng.random_range(0..time_range_seconds);
            let timestamp = start_time + Duration::seconds(random_seconds);
            let metadata = Self::generate_metadata_static();

            let active_event = events::ActiveModel {
                id: Set(Uuid::now_v7()),
                user_id: Set(user_id),
                event_type_id: Set(event_type_id),
                timestamp: Set(timestamp),
                metadata: Set(Some(metadata)),
            };

            all_events.push(active_event);
        }

        Ok(all_events)
    }

    // Phase 2: Pure database insert (I/O intensive, concurrent)
    #[instrument(skip(db, events_batch, progress_tracker))]
    async fn insert_pre_generated_batch(
        db: &SqlConnect, events_batch: Vec<events::ActiveModel>,
        batch_start: usize, target_events: usize,
        progress_tracker: &Option<ProgressTracker>,
    ) -> Result<()> {
        let db_conn = db.get_connect();
        let batch_size = events_batch.len();

        // Pure insert operation - no data generation overhead
        events::Entity::insert_many(events_batch)
            .exec(db_conn)
            .await?;

        // Update progress
        if let Some(tracker) = progress_tracker {
            let current_total = batch_start + batch_size;
            let progress_percentage =
                (current_total as f64 / target_events as f64) * 100.0;

            tracker.update(ProgressUpdate {
                seeder_name: "EventSeeder".to_string(),
                current: current_total,
                total: target_events,
                message: format!(
                    "Inserted {} events ({:.1}% complete)",
                    current_total, progress_percentage
                ),
            });
        }

        Ok(())
    }

    #[instrument(skip(self, progress_tracker))]
    async fn generate_events(
        &self, user_ids: Vec<Uuid>, event_type_ids: Vec<i32>,
        progress_tracker: &Option<ProgressTracker>,
    ) -> Result<()> {
        // Use parallel processing for better performance
        self.generate_events_parallel(
            user_ids,
            event_type_ids,
            progress_tracker,
        )
        .await
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
