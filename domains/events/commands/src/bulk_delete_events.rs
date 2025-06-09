use chrono::{DateTime, Utc};
use events_dao::EventDao;
use serde::{Deserialize, Serialize};
use sql_connection::SqlConnect;
use thiserror::Error;
use tracing::instrument;
use utoipa::ToSchema;

#[derive(Debug, Error)]
pub enum BulkDeleteEventsError {
    #[error("Event DAO error: {0}")]
    EventDao(#[from] events_dao::EventDaoError),
    #[error("Invalid timestamp: timestamp must be in the past")]
    InvalidTimestamp,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct BulkDeleteEventsCommand {
    pub before: DateTime<Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct BulkDeleteEventsResponse {
    pub deleted_count: u64,
    pub deleted_before: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct BulkDeleteEventsResult {
    pub result: BulkDeleteEventsResponse,
}

#[derive(Clone)]
pub struct BulkDeleteEventsHandler {
    event_dao: EventDao,
}

impl BulkDeleteEventsHandler {
    pub fn new(db: SqlConnect) -> Self {
        Self {
            event_dao: EventDao::new(db),
        }
    }

    #[instrument(skip(self))]
    pub async fn execute(
        &self, command: BulkDeleteEventsCommand,
    ) -> Result<BulkDeleteEventsResult, BulkDeleteEventsError> {
        // Validate that the timestamp is in the past
        if command.before > Utc::now() {
            return Err(BulkDeleteEventsError::InvalidTimestamp);
        }

        let deleted_count = self
            .event_dao
            .delete_before_timestamp(command.before)
            .await?;

        Ok(BulkDeleteEventsResult {
            result: BulkDeleteEventsResponse {
                deleted_count,
                deleted_before: command.before,
            },
        })
    }
}
