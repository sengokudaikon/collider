use anyhow::Result;
use async_trait::async_trait;
use database_traits::connection::GetDatabaseConnect;
use events_models::event_types;
use fake::{Fake, faker::lorem::en::Word};
use rand::{Rng, rng};
use sea_orm::{EntityTrait, Set};
use sql_connection::SqlConnect;
use tracing::{info, instrument};

use crate::Seeder;

pub struct EventTypeSeeder {
    db: SqlConnect,
    min_types: usize,
    max_types: usize,
    batch_size: usize,
}

impl EventTypeSeeder {
    pub fn new(db: SqlConnect, min_types: usize, max_types: usize) -> Self {
        let batch_size = Self::calculate_batch_size(min_types, max_types);
        Self {
            db,
            min_types,
            max_types,
            batch_size,
        }
    }

    fn calculate_batch_size(min_types: usize, max_types: usize) -> usize {
        let expected_avg = (min_types + max_types) / 2;
        match expected_avg {
            0..=50 => 25,
            51..=200 => 50,
            201..=1000 => 100,
            _ => 200,
        }
    }

    #[instrument(skip(self), fields(batch_size = self.batch_size))]
    async fn generate_event_type_batch(
        &self, batch_size: usize,
    ) -> Result<Vec<String>> {
        let db = self.db.get_connect();
        let mut batch_event_types = Vec::with_capacity(batch_size);
        let mut event_names = Vec::with_capacity(batch_size);

        let prefixes = [
            "user",
            "page",
            "button",
            "form",
            "video",
            "purchase",
            "signup",
            "login",
            "click",
            "view",
            "download",
            "share",
            "comment",
            "like",
            "search",
            "filter",
            "cart",
            "checkout",
            "payment",
            "notification",
            "error",
            "warning",
            "info",
        ];

        for _ in 0..batch_size {
            let prefix = {
                let mut rng = rng();
                prefixes[rng.random_range(0..prefixes.len())]
            };
            let suffix: String = Word().fake();
            let event_name = format!("{}_{}", prefix, suffix.to_lowercase());

            let active_event_type = event_types::ActiveModel {
                id: sea_orm::NotSet,
                name: Set(event_name.clone()),
            };

            batch_event_types.push(active_event_type);
            event_names.push(event_name);
        }

        event_types::Entity::insert_many(batch_event_types)
            .exec(db)
            .await?;
        Ok(event_names)
    }

    #[instrument(skip(self))]
    async fn generate_event_types(
        &self, count: usize,
    ) -> Result<Vec<String>> {
        info!(
            "Generating {} event types in batches of {}",
            count, self.batch_size
        );

        let mut all_event_names = Vec::with_capacity(count);
        let total_batches = count.div_ceil(self.batch_size);

        for batch_num in 0..total_batches {
            let batch_start = batch_num * self.batch_size;
            let remaining_types = count - batch_start;
            let current_batch_size =
                std::cmp::min(self.batch_size, remaining_types);

            if current_batch_size == 0 {
                break;
            }

            let batch_event_names =
                self.generate_event_type_batch(current_batch_size).await?;
            all_event_names.extend(batch_event_names);

            let current_total = batch_start + current_batch_size;
            if current_total % 100 == 0 || current_total == count {
                info!("Generated {} event types", current_total);
            }
        }

        Ok(all_event_names)
    }
}

#[async_trait]
impl Seeder for EventTypeSeeder {
    async fn seed(&self) -> Result<()> {
        let type_count = {
            let mut rng = rng();
            rng.random_range(self.min_types..=self.max_types)
        };

        info!("Seeding {} event types (random smaller count)", type_count);

        let _event_names = self.generate_event_types(type_count).await?;

        info!("Successfully seeded {} event types", type_count);
        Ok(())
    }

    fn name(&self) -> &'static str { "EventTypeSeeder" }
}
