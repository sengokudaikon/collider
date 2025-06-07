use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use database_traits::connection::GetDatabaseConnect;
use fake::{Fake, faker::name::en::Name};
use rand::{Rng, rng};
use sea_orm::{ActiveModelTrait, Set};
use sql_connection::SqlConnect;
use tracing::{info, instrument};
use user_models as users;
use uuid::Uuid;

use crate::Seeder;

pub struct UserSeeder {
    db: SqlConnect,
    min_users: usize,
    max_users: usize,
}

impl UserSeeder {
    pub fn new(db: SqlConnect, min_users: usize, max_users: usize) -> Self {
        Self {
            db,
            min_users,
            max_users,
        }
    }

    #[instrument(skip(self))]
    async fn generate_users(&self, count: usize) -> Result<Vec<Uuid>> {
        let db = self.db.get_connect();
        let mut user_ids = Vec::with_capacity(count);

        info!("Generating {} users", count);

        for i in 0..count {
            let user_id = Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext));
            let fake_name: String = Name().fake();

            let active_user = users::ActiveModel {
                id: Set(user_id),
                name: Set(fake_name),
                created_at: Set(Utc::now()),
            };

            active_user.insert(db).await?;
            user_ids.push(user_id);

            if (i + 1) % 10000 == 0 {
                info!("Generated {} users", i + 1);
            }
        }

        Ok(user_ids)
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
