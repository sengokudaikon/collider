use std::env;

use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use phf_macros::phf_map;
use rand::{Rng, SeedableRng, rngs::SmallRng, thread_rng};
use serde_json::Value;
use tokio_postgres::NoTls;

pub(crate) fn timestamp() -> String {
    Utc::now().format("[%H:%M:%S]").to_string()
}

#[derive(Debug, Clone)]
pub struct User {
    pub id: i64,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct EventType {
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct Event {
    pub user_id: i64,
    pub event_type: String,
    pub timestamp: DateTime<Utc>,
    pub metadata: Value,
}

pub static REFERRERS: &[&str] = &[
    "https://google.com",
    "https://facebook.com",
    "https://youtube.com",
    "https://twitter.com",
    "https://instagram.com",
    "https://tiktok.com",
    "https://linkedin.com",
    "https://reddit.com",
    "https://pinterest.com",
    "https://yahoo.com",
    "https://bing.com",
    "https://amazon.com",
    "https://ebay.com",
    "https://wikipedia.org",
    "https://medium.com",
    "https://quora.com",
    "https://stackoverflow.com",
    "https://github.com",
    "https://netflix.com",
    "https://news.ycombinator.com",
];

pub static EVENT_TYPES: phf::Map<
    &'static str,
    phf::Map<&'static str, &'static str>,
> = phf_map! {
    "user.registered" => phf_map!{"page" => "/registration"},
    "user.login" => phf_map!{"page" => "/login"},
    "user.logout" => phf_map!{"page" => "/logout"},
    "user.updated" => phf_map!{"page" => "/profile/edit"},
    "order.created" => phf_map!{"page" => "/order/create"},
    "order.paid" => phf_map!{"page" => "/order/confirm"},
    "order.shipped" => phf_map!{"page" => "/order/shipped"},
    "order.delivered" => phf_map!{"page" => "/order/tracking"},
    "payment.processed" => phf_map!{"page" => "/payment/complete"},
    "payment.failed" => phf_map!{"page" => "/payment/failed"},
    "payment.refunded" => phf_map!{"page" => "/payment/refund"},
    "product.viewed" => phf_map!{"page" => "/product/view"},
    "product.added_to_cart" => phf_map!{"page" => "/cart/add"},
    "product.removed_from_cart" => phf_map!{"page" => "/cart/remove"},
    "email.sent" => phf_map!{"page" => "/emails/sent"},
    "email.opened" => phf_map!{"page" => "/emails/opened"},
    "email.clicked" => phf_map!{"page" => "/emails/click"},
    "notification.sent" => phf_map!{"page" => "/notifications/sent"},
    "notification.read" => phf_map!{"page" => "/notifications/read"},
    "api.request" => phf_map!{"page" => "/api/request"},
    "api.response" => phf_map!{"page" => "/api/response"},
    "api.error" => phf_map!{"page" => "/api/error"},
    "user.password_reset_requested" => phf_map!{"page" => "/password/reset"},
    "user.password_changed" => phf_map!{"page" => "/password/change"},
    "user.two_factor_enabled" => phf_map!{"page" => "/security/2fa"},
    "user.two_factor_disabled" => phf_map!{"page" => "/security/2fa/disable"},
    "user.deleted" => phf_map!{"page" => "/account/delete"},
    "user.suspended" => phf_map!{"page" => "/account/suspend"},
    "user.reactivated" => phf_map!{"page" => "/account/reactivate"},
    "user.subscription_started" => phf_map!{"page" => "/subscription/start"},
    "user.subscription_cancelled" => phf_map!{"page" => "/subscription/cancel"},
    "user.subscription_renewed" => phf_map!{"page" => "/subscription/renew"},
    "user.invited" => phf_map!{"page" => "/invite/send"},
    "user.invite_accepted" => phf_map!{"page" => "/invite/accept"},
    "user.feedback_submitted" => phf_map!{"page" => "/feedback"},
    "user.avatar_uploaded" => phf_map!{"page" => "/profile/avatar"},
    "user.preferences_updated" => phf_map!{"page" => "/profile/preferences"},
    "user.email_verified" => phf_map!{"page" => "/email/verify"},
    "user.login_failed" => phf_map!{"page" => "/login/failed"},
    "user.profile_viewed" => phf_map!{"page" => "/profile/view"},
    "user.notification_preferences_updated" => phf_map!{"page" => "/profile/notifications"},
    "user.newsletter_subscribed" => phf_map!{"page" => "/newsletter/subscribe"},
    "order.cancelled" => phf_map!{"page" => "/order/cancel"},
    "order.return_requested" => phf_map!{"page" => "/order/return"},
    "order.return_approved" => phf_map!{"page" => "/order/return/approved"},
    "order.return_rejected" => phf_map!{"page" => "/order/return/rejected"},
    "order.review_submitted" => phf_map!{"page" => "/order/review"},
    "order.invoice_generated" => phf_map!{"page" => "/order/invoice"},
    "payment.pending" => phf_map!{"page" => "/payment/pending"},
    "payment.disputed" => phf_map!{"page" => "/payment/dispute"},
    "payment.settled" => phf_map!{"page" => "/payment/settled"},
    "cart.viewed" => phf_map!{"page" => "/cart/view"},
    "cart.updated" => phf_map!{"page" => "/cart/update"},
    "cart.cleared" => phf_map!{"page" => "/cart/clear"},
    "checkout.started" => phf_map!{"page" => "/checkout/start"},
    "checkout.completed" => phf_map!{"page" => "/checkout/complete"},
    "product.review_submitted" => phf_map!{"page" => "/product/review"},
    "product.wishlisted" => phf_map!{"page" => "/wishlist/add"},
    "product.unwishlisted" => phf_map!{"page" => "/wishlist/remove"},
    "product.compared" => phf_map!{"page" => "/product/compare"},
    "product.shared" => phf_map!{"page" => "/product/share"},
    "product.restock_requested" => phf_map!{"page" => "/product/restock"},
    "product.stock_low" => phf_map!{"page" => "/product/stock"},
    "email.bounced" => phf_map!{"page" => "/emails/bounced"},
    "email.unsubscribed" => phf_map!{"page" => "/emails/unsubscribe"},
    "notification.dismissed" => phf_map!{"page" => "/notifications/dismiss"},
    "notification.failed" => phf_map!{"page" => "/notifications/failure"},
    "session.started" => phf_map!{"page" => "/session/start"},
    "session.expired" => phf_map!{"page" => "/session/expired"},
    "session.terminated" => phf_map!{"page" => "/session/end"},
    "admin.login" => phf_map!{"page" => "/admin/login"},
    "admin.logout" => phf_map!{"page" => "/admin/logout"},
    "admin.updated_user" => phf_map!{"page" => "/admin/user/edit"},
    "admin.deleted_user" => phf_map!{"page" => "/admin/user/delete"},
    "admin.generated_report" => phf_map!{"page" => "/admin/reports"},
    "admin.settings_updated" => phf_map!{"page" => "/admin/settings"},
    "file.uploaded" => phf_map!{"page" => "/files/upload"},
    "file.deleted" => phf_map!{"page" => "/files/delete"},
    "file.downloaded" => phf_map!{"page" => "/files/download"},
    "file.previewed" => phf_map!{"page" => "/files/preview"},
    "support.ticket_created" => phf_map!{"page" => "/support/create"},
    "support.ticket_closed" => phf_map!{"page" => "/support/close"},
    "support.ticket_reopened" => phf_map!{"page" => "/support/reopen"},
    "support.message_sent" => phf_map!{"page" => "/support/message"},
    "support.rating_submitted" => phf_map!{"page" => "/support/rating"},
    "search.performed" => phf_map!{"page" => "/search"},
    "search.filtered" => phf_map!{"page" => "/search/filter"},
    "search.sorted" => phf_map!{"page" => "/search/sort"},
    "settings.updated" => phf_map!{"page" => "/settings"},
    "language.changed" => phf_map!{"page" => "/settings/language"},
    "timezone.changed" => phf_map!{"page" => "/settings/timezone"},
    "api.token_generated" => phf_map!{"page" => "/api/token"},
    "api.token_revoked" => phf_map!{"page" => "/api/token/revoke"},
    "api.rate_limited" => phf_map!{"page" => "/api/rate-limit"},
    "cron.job_started" => phf_map!{"page" => "/cron/start"},
    "cron.job_finished" => phf_map!{"page" => "/cron/end"},
    "cron.job_failed" => phf_map!{"page" => "/cron/failure"},
    "webhook.received" => phf_map!{"page" => "/webhooks/incoming"},
    "webhook.verified" => phf_map!{"page" => "/webhooks/verified"},
    "webhook.failed" => phf_map!{"page" => "/webhooks/failure"},
};
pub fn create_users(count: usize) -> Vec<User> {
    let created_at = Utc::now();

    (0..count)
        .map(|i| {
            User {
                id: 0, // Will be set after insert
                name: format!("User{}", i + 1),
                created_at,
            }
        })
        .collect()
}

pub fn create_event_types(count: usize) -> Vec<EventType> {
    use rand::{seq::SliceRandom, thread_rng};

    let mut rng = thread_rng();
    let base_types: Vec<&str> = EVENT_TYPES.keys().cloned().collect();

    // If we need more event types than available, repeat and randomize
    let mut event_types = Vec::with_capacity(count);

    for i in 0..count {
        let base_name = base_types[i % base_types.len()];

        // Add some randomization to make event types unique
        let randomized_name = if i < base_types.len() {
            // For the first round, use original names
            base_name.to_string()
        }
        else {
            // For subsequent rounds, add a suffix
            let suffixes = [
                "_v2",
                "_alt",
                "_new",
                "_extended",
                "_pro",
                "_lite",
                "_plus",
                "_max",
            ];
            let suffix = suffixes.choose(&mut rng).unwrap();
            format!("{base_name}{suffix}")
        };

        event_types.push(EventType {
            name: randomized_name,
        });
    }

    event_types
}

/// Generate a specified number of events with realistic metadata
pub fn create_events(
    count: usize, users: &[User], event_types: &[EventType],
) -> Vec<Event> {
    let now = Utc::now();
    let thirty_days_ago = now - Duration::days(30);
    let time_range_seconds = (now - thirty_days_ago).num_seconds();

    (0..count)
        .map(|i| {
            let mut rng = SmallRng::from_entropy();

            // Select random user and event type
            let user = &users[i % users.len()];
            let event_type = &event_types[i % event_types.len()];

            // Generate random timestamp within the last 30 days
            let random_seconds = rng.gen_range(0..time_range_seconds);
            let timestamp = thirty_days_ago + Duration::seconds(random_seconds);

            // Pre-generated metadata compatible with materialized views
            let category = event_type.name.split('.').next().unwrap_or("unknown");
            let metadata = match category {
                "product" | "order" | "cart" | "checkout" => serde_json::json!({
                    "page": EVENT_TYPES.get(&event_type.name).and_then(|m| m.get("page")).map_or("/unknown", |v| *v),
                    "product_id": rng.gen_range(1..=5000),
                    "referrer": REFERRERS[rng.gen_range(0..REFERRERS.len())],
                    "session_id": rng.gen_range(100_000_000..999_999_999).to_string()
                }),
                _ => serde_json::json!({
                    "page": EVENT_TYPES.get(&event_type.name).and_then(|m| m.get("page")).map_or("/unknown", |v| *v),
                    "referrer": REFERRERS[rng.gen_range(0..REFERRERS.len())], 
                    "session_id": rng.gen_range(100_000_000..999_999_999).to_string()
                })
            };

            Event {
                user_id: user.id,
                event_type: event_type.name.clone(),
                timestamp,
                metadata,
            }
        })
        .collect()
}

/// Generate a specific batch of events for just-in-time processing
/// This is optimized for memory efficiency by generating only one batch at a
/// time
pub fn create_events_for_batch(
    count: usize,
    user_ids: &[i64],
    event_types: &[EventType],
    offset: usize, // The starting index for this batch
) -> Vec<Event> {
    use rayon::prelude::*;

    let now = Utc::now();
    let thirty_days_ago = now - Duration::days(30);
    let time_range_seconds = (now - thirty_days_ago).num_seconds();

    // Use parallel processing for a single batch to maintain performance
    (0..count)
        .into_par_iter()
        .map(|i| {
            let mut rng = SmallRng::from_rng(thread_rng()).unwrap();
            let event_index = offset + i; // Global event index

            // Select user and event type using global index for consistency
            let user_id = user_ids[event_index % user_ids.len()];
            let event_type = &event_types[event_index % event_types.len()];

            // Generate random timestamp within the last 30 days
            let random_seconds = rng.gen_range(0..time_range_seconds);
            let timestamp = thirty_days_ago + Duration::seconds(random_seconds);

            // Pre-generated metadata compatible with materialized views
            let category = event_type.name.split('.').next().unwrap_or("unknown");
            let metadata = match category {
                "product" | "order" | "cart" | "checkout" => serde_json::json!({
                    "page": EVENT_TYPES.get(&event_type.name).and_then(|m| m.get("page")).map_or("/unknown", |v| *v),
                    "product_id": rng.gen_range(1..=5000),
                    "referrer": REFERRERS[rng.gen_range(0..REFERRERS.len())],
                    "session_id": rng.gen_range(100_000_000..999_999_999).to_string()
                }),
                _ => serde_json::json!({
                    "page": EVENT_TYPES.get(&event_type.name).and_then(|m| m.get("page")).map_or("/unknown", |v| *v),
                    "referrer": REFERRERS[rng.gen_range(0..REFERRERS.len())], 
                    "session_id": rng.gen_range(100_000_000..999_999_999).to_string()
                })
            };

            Event {
                user_id,
                event_type: event_type.name.clone(),
                timestamp,
                metadata,
            }
        })
        .collect()
}

pub async fn prepare_database(pool: &Pool) -> anyhow::Result<()> {
    let client = pool.get().await?;

    println!(
        "{} 🗑️  Dropping performance indexes for fast bulk inserts...",
        timestamp()
    );

    client
        .batch_execute(
            "
             SET session_replication_role = replica;
             SET synchronous_commit = OFF;
             SET commit_delay = 100000;
             
             ALTER TABLE events DISABLE TRIGGER ALL;
             ALTER TABLE users DISABLE TRIGGER ALL;
             ALTER TABLE event_types DISABLE TRIGGER ALL;

             DROP INDEX IF EXISTS idx_events_user_id;
             DROP INDEX IF EXISTS idx_events_timestamp; 
             DROP INDEX IF EXISTS idx_events_user_id_timestamp;
             DROP INDEX IF EXISTS idx_events_event_type_id;
             DROP INDEX IF EXISTS idx_events_metadata_gin;
             
             TRUNCATE events, users, event_types RESTART IDENTITY CASCADE;",
        )
        .await?;

    println!(
        "{} ✅ Database prepared for bulk inserts (indexes dropped)",
        timestamp()
    );
    Ok(())
}

pub async fn restore_database(pool: &Pool) -> anyhow::Result<()> {
    let client = pool.get().await?;

    println!("{} 🔧 Recreating performance indexes...", timestamp());

    client
        .batch_execute(
            "
             ALTER TABLE events ENABLE TRIGGER ALL;
             ALTER TABLE users ENABLE TRIGGER ALL; 
             ALTER TABLE event_types ENABLE TRIGGER ALL;
             
             SET session_replication_role = DEFAULT;
             SET synchronous_commit = ON;",
        )
        .await?;

    // Recreate performance indexes one by one with progress reporting
    let indexes = [
        (
            "idx_events_user_id",
            "CREATE INDEX idx_events_user_id ON events (user_id)",
        ),
        (
            "idx_events_timestamp",
            "CREATE INDEX idx_events_timestamp ON events (timestamp DESC)",
        ),
        (
            "idx_events_user_id_timestamp",
            "CREATE INDEX idx_events_user_id_timestamp ON events (user_id, \
             timestamp DESC)",
        ),
        (
            "idx_events_event_type_id",
            "CREATE INDEX idx_events_event_type_id ON events (event_type_id)",
        ),
        (
            "idx_events_metadata_gin",
            "CREATE INDEX idx_events_metadata_gin ON events USING GIN \
             (metadata)",
        ),
    ];

    for (name, sql) in indexes {
        println!("{}   Creating index: {name}", timestamp());
        let start = std::time::Instant::now();

        if let Err(e) = client.execute(sql, &[]).await {
            println!(
                "{}   ⚠️  Warning: Failed to create {name}: {e}",
                timestamp()
            );
        }
        else {
            println!(
                "{}   ✅ Created {} in {:?}",
                timestamp(),
                name,
                start.elapsed()
            );
        }
    }

    println!(
        "{} 🚀 Database restored with all performance indexes",
        timestamp()
    );
    println!("{} Refreshing materialized views...", timestamp());
    client
        .execute("REFRESH MATERIALIZED VIEW stats_summary;", &[])
        .await?;
    println!("{} ✅ Materialized views refreshed", timestamp());
    Ok(())
}

pub async fn create_pool() -> anyhow::Result<Pool> {
    let pg_cfg =
        env::var("DATABASE_URL")?.parse::<tokio_postgres::Config>()?;

    let mgr_config = ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    };

    let mgr = Manager::from_config(pg_cfg, NoTls, mgr_config);

    let pool = Pool::builder(mgr)
        .max_size(300)
        .runtime(deadpool_postgres::Runtime::Tokio1)
        .build()?;

    Ok(pool)
}
