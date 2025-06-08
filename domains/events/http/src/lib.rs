pub mod handlers;

pub use events_models::EventResponse;
pub use handlers::*;

use axum::Router;
use sql_connection::SqlConnect;

pub fn event_routes() -> Router {
    let db = SqlConnect::from_global();
    let services = handlers::EventServices::new(db);
    handlers::EventHandlers::routes().with_state(services)
}
