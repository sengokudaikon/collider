pub mod handlers;

use axum::Router;
pub use handlers::*;
use sql_connection::SqlConnect;

pub fn analytics_routes() -> Router {
    let db = SqlConnect::from_global();
    let services = handlers::AnalyticsServices::new(db);
    handlers::AnalyticsHandlers::routes().with_state(services)
}
