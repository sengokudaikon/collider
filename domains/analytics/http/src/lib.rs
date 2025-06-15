pub mod handlers;

use analytics::RedisAnalyticsMetricsUpdater;
use axum::Router;
pub use handlers::*;
use sql_connection::SqlConnect;

pub fn analytics_routes() -> Router {
    let db = SqlConnect::from_global();
    let redis_updater = RedisAnalyticsMetricsUpdater::new();

    let services = handlers::AnalyticsServices::new(db, redis_updater);

    handlers::AnalyticsHandlers::routes().with_state(services)
}
