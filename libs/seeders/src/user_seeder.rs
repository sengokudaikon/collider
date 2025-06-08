use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use database_traits::connection::GetDatabaseConnect;
use fake::{Fake, faker::name::en::Name};
use rand::{Rng, rng};
use sea_orm::{EntityTrait, Set};
use sql_connection::SqlConnect;
use tracing::{info, instrument};
use user_models as users;
use uuid::Uuid;

use crate::Seeder;

pub struct UserSeeder {
    db: SqlConnect,
    min_users: usize,
    max_users: usize,
    batch_size: usize,
}

impl UserSeeder {
    pub fn new(db: SqlConnect, min_users: usize, max_users: usize) -> Self {
        let batch_size = Self::calculate_batch_size(min_users, max_users);
        Self {
            db,
            min_users,
            max_users,
            batch_size,
        }
    }

    fn calculate_batch_size(min_users: usize, max_users: usize) -> usize {
        let expected_avg = (min_users + max_users) / 2;
        match expected_avg {
            0..=100 => 50,
            101..=1000 => 100,
            1001..=10000 => 500,
            10001..=100000 => 1000,
            _ => 2000,
        }
    }

    #[instrument(skip(self), fields(batch_size = self.batch_size))]
    async fn generate_user_batch(
        &self, batch_size: usize,
    ) -> Result<Vec<Uuid>> {
        let db = self.db.get_connect();
        let mut batch_users = Vec::with_capacity(batch_size);
        let mut user_ids = Vec::with_capacity(batch_size);

        for _ in 0..batch_size {
            let user_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
            let fake_name: String = Name().fake();

            let active_user = users::ActiveModel {
                id: Set(user_id),
                name: Set(fake_name),
                created_at: Set(Utc::now()),
            };

            batch_users.push(active_user);
            user_ids.push(user_id);
        }

        users::Entity::insert_many(batch_users).exec(db).await?;
        Ok(user_ids)
    }

    #[instrument(skip(self))]
    async fn generate_users(&self, count: usize) -> Result<Vec<Uuid>> {
        info!(
            "Generating {} users in batches of {}",
            count, self.batch_size
        );

        let mut all_user_ids = Vec::with_capacity(count);
        let total_batches = count.div_ceil(self.batch_size);

        for batch_num in 0..total_batches {
            let batch_start = batch_num * self.batch_size;
            let remaining_users = count - batch_start;
            let current_batch_size =
                std::cmp::min(self.batch_size, remaining_users);

            if current_batch_size == 0 {
                break;
            }

            let batch_user_ids =
                self.generate_user_batch(current_batch_size).await?;
            all_user_ids.extend(batch_user_ids);

            let current_total = batch_start + current_batch_size;
            if current_total % 10000 == 0 || current_total == count {
                info!("Generated {} users", current_total);
            }
        }

        Ok(all_user_ids)
    }
}

#[async_trait]
impl Seeder for UserSeeder {
    async fn seed(&self) -> Result<()> {
        let user_count = {
            let mut rng = rng();
            rng.random_range(self.min_users..=self.max_users)
        };

        info!("Seeding {} users (random entropic count)", user_count);

        let _user_ids = self.generate_users(user_count).await?;

        info!("Successfully seeded {} users", user_count);
        Ok(())
    }

    fn name(&self) -> &'static str { "UserSeeder" }
}
