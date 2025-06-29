use events_errors::EventTypeError;
use events_models::{
    CreateEventTypeRequest, EventTypeResponse, UpdateEventTypeRequest,
};
use sql_connection::SqlConnect;
use tracing::instrument;

#[derive(Clone)]
pub struct EventTypeDao {
    db: SqlConnect,
}

impl EventTypeDao {
    pub fn new(db: SqlConnect) -> Self { Self { db } }
}

impl EventTypeDao {
    #[instrument(skip_all)]
    pub async fn find_by_id(
        &self, id: i32,
    ) -> Result<EventTypeResponse, EventTypeError> {
        let client = self.db.get_read_client().await?;
        let stmt = client
            .prepare("SELECT id, name FROM event_types WHERE id = $1")
            .await?;
        let rows = client.query(&stmt, &[&id]).await?;

        let event_type = rows
            .first()
            .map(|row| {
                EventTypeResponse {
                    id: row.get(0),
                    name: row.get(1),
                }
            })
            .ok_or(EventTypeError::NotFound)?;

        Ok(event_type)
    }

    #[instrument(skip_all)]
    pub async fn all(
        &self,
    ) -> Result<Vec<EventTypeResponse>, EventTypeError> {
        let client = self.db.get_read_client().await?;
        let stmt = client
            .prepare("SELECT id, name FROM event_types ORDER BY name ASC")
            .await?;
        let rows = client.query(&stmt, &[]).await?;

        let event_types = rows
            .iter()
            .map(|row| {
                EventTypeResponse {
                    id: row.get(0),
                    name: row.get(1),
                }
            })
            .collect();

        Ok(event_types)
    }

    #[instrument(skip_all)]
    pub async fn create(
        &self, req: CreateEventTypeRequest,
    ) -> Result<EventTypeResponse, EventTypeError> {
        let client = self.db.get_client().await?;

        // Check if name already exists
        let check_stmt = client
            .prepare("SELECT id FROM event_types WHERE name = $1")
            .await?;
        let check_rows = client.query(&check_stmt, &[&req.name]).await?;
        if !check_rows.is_empty() {
            return Err(EventTypeError::AlreadyExists);
        }

        let stmt = client
            .prepare(
                "INSERT INTO event_types (name) VALUES ($1) RETURNING id, \
                 name",
            )
            .await?;
        let rows = client.query(&stmt, &[&req.name]).await?;

        let event_type = rows
            .first()
            .map(|row| {
                EventTypeResponse {
                    id: row.get(0),
                    name: row.get(1),
                }
            })
            .ok_or_else(|| {
                EventTypeError::InternalError(
                    "No row returned from INSERT".to_string(),
                )
            })?;

        Ok(event_type)
    }

    #[instrument(skip_all)]
    pub async fn update(
        &self, id: i32, req: UpdateEventTypeRequest,
    ) -> Result<EventTypeResponse, EventTypeError> {
        let client = self.db.get_client().await?;

        // Check if event type exists
        let check_stmt = client
            .prepare("SELECT id FROM event_types WHERE id = $1")
            .await?;
        let check_rows = client.query(&check_stmt, &[&id]).await?;
        if check_rows.is_empty() {
            return Err(EventTypeError::NotFound);
        }

        if let Some(name) = req.name {
            // Check if new name already exists for a different event type
            let check_name_stmt = client
                .prepare(
                    "SELECT id FROM event_types WHERE name = $1 AND id != $2",
                )
                .await?;
            let check_name_rows =
                client.query(&check_name_stmt, &[&name, &id]).await?;
            if !check_name_rows.is_empty() {
                return Err(EventTypeError::AlreadyExists);
            }

            let stmt = client
                .prepare(
                    "UPDATE event_types SET name = $1 WHERE id = $2 \
                     RETURNING id, name",
                )
                .await?;
            let rows = client.query(&stmt, &[&name, &id]).await?;

            let event_type = rows
                .first()
                .map(|row| {
                    EventTypeResponse {
                        id: row.get(0),
                        name: row.get(1),
                    }
                })
                .ok_or(EventTypeError::NotFound)?;

            Ok(event_type)
        }
        else {
            // No update needed, just return the existing event type
            let stmt = client
                .prepare("SELECT id, name FROM event_types WHERE id = $1")
                .await?;
            let rows = client.query(&stmt, &[&id]).await?;

            let event_type = rows
                .first()
                .map(|row| {
                    EventTypeResponse {
                        id: row.get(0),
                        name: row.get(1),
                    }
                })
                .ok_or(EventTypeError::NotFound)?;

            Ok(event_type)
        }
    }

    #[instrument(skip_all)]
    pub async fn delete(&self, id: i32) -> Result<(), EventTypeError> {
        let client = self.db.get_client().await?;
        let stmt = client
            .prepare("DELETE FROM event_types WHERE id = $1")
            .await?;
        let affected = client.execute(&stmt, &[&id]).await?;

        if affected == 0 {
            return Err(EventTypeError::NotFound);
        }

        Ok(())
    }

    #[instrument(skip_all)]
    pub async fn find_by_name(
        &self, name: &str,
    ) -> Result<EventTypeResponse, EventTypeError> {
        let client = self.db.get_read_client().await?;
        let stmt = client
            .prepare("SELECT id, name FROM event_types WHERE name = $1")
            .await?;
        let rows = client.query(&stmt, &[&name]).await?;

        let event_type = rows
            .first()
            .map(|row| {
                EventTypeResponse {
                    id: row.get(0),
                    name: row.get(1),
                }
            })
            .ok_or(EventTypeError::NotFound)?;

        Ok(event_type)
    }
}
