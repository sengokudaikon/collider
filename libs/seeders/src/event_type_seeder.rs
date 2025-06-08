use anyhow::Result;
use async_trait::async_trait;
use database_traits::connection::GetDatabaseConnect;
use events_models::event_types;
use fake::{Fake, faker::lorem::en::Word};
use rand::{Rng, rng};
use sea_orm::{ActiveModelTrait, Set};
use sql_connection::SqlConnect;
use tracing::{info, instrument};

use crate::Seeder;

pub struct EventTypeSeeder {
    db: SqlConnect,
    min_types: usize,
    max_types: usize,
}

impl EventTypeSeeder {
    pub fn new(db: SqlConnect, min_types: usize, max_types: usize) -> Self {
        Self {
            db,
            min_types,
            max_types,
        }
    }

    #[instrument(skip(self))]
    async fn generate_event_types(&self, count: usize) -> Result<Vec<i32>> {
        let db = self.db.get_connect();
        let mut event_type_ids = Vec::with_capacity(count);

        info!("Generating {} event types", count);

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

        for i in 0..count {
            let prefix = {
                let mut rng = rng();
                prefixes[rng.random_range(0..prefixes.len())]
            };
            let suffix: String = Word().fake();
            let event_name = format!("{}_{}", prefix, suffix.to_lowercase());

            let active_event_type = event_types::ActiveModel {
                id: sea_orm::NotSet,
                name: Set(event_name),
            };

            let result = active_event_type.insert(db).await?;
            event_type_ids.push(result.id);

            if (i + 1) % 100 == 0 {
                info!("Generated {} event types", i + 1);
            }
        }

        Ok(event_type_ids)
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

        let _event_type_ids = self.generate_event_types(type_count).await?;

        info!("Successfully seeded {} event types", type_count);
        Ok(())
    }

    fn name(&self) -> &'static str { "EventTypeSeeder" }
}
