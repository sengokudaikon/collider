use async_trait::async_trait;
use chrono::{DateTime, Utc};
use dao_utils::query_helpers::{PgParam, PgParamVec};
use database_traits::dao::GenericDao;
use events_commands::{CreateEventCommand, UpdateEventCommand};
use events_errors::{EventError, EventTypeError};
use events_models::Event;
use events_responses::EventResponse;
use sql_connection::SqlConnect;
use tracing::instrument;


#[derive(Clone)]
pub struct EventDao {
    db: SqlConnect,
}

impl EventDao {
    pub fn new(db: SqlConnect) -> Self { Self { db } }

    pub fn db(&self) -> &SqlConnect { &self.db }

    fn map_row_to_response(&self, row: &tokio_postgres::Row) -> EventResponse {
        let metadata_json: Option<serde_json::Value> = row.get(4);
        let metadata =
            metadata_json.and_then(|json| serde_json::from_value(json).ok());

        EventResponse {
            id: row.get(0),
            user_id: row.get(1),
            event_type_id: row.get(2),
            event_type: row.get(5), // event type name from JOIN
            timestamp: row.get(3),
            metadata,
        }
    }

    #[instrument(skip(self))]
    pub async fn count_events(
        &self, from: DateTime<Utc>, to: DateTime<Utc>,
        event_type: Option<String>,
    ) -> Result<i64, EventError> {
        let client = self.db.get_read_client().await?;

        let (query, params): (&str, PgParamVec) =
            if let Some(event_type) = event_type {
                (
                    "SELECT COUNT(*) FROM events e 
                 JOIN event_types et ON e.event_type_id = et.id 
                 WHERE e.timestamp >= $1 AND e.timestamp <= $2 AND et.name = \
                     $3",
                    vec![Box::new(from), Box::new(to), Box::new(event_type)],
                )
            }
            else {
                (
                    "SELECT COUNT(*) FROM events WHERE timestamp >= $1 AND \
                     timestamp <= $2",
                    vec![Box::new(from), Box::new(to)],
                )
            };

        let stmt = client.prepare(query).await?;
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params
                .iter()
                .map(|p| {
                    p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)
                })
                .collect();

        let row = client.query_one(&stmt, &param_refs).await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self))]
    pub async fn count_unique_users(
        &self, from: DateTime<Utc>, to: DateTime<Utc>,
        event_type: Option<String>,
    ) -> Result<i64, EventError> {
        let client = self.db.get_read_client().await?;

        let (query, params): (&str, PgParamVec) =
            if let Some(event_type) = event_type {
                (
                    "SELECT COUNT(DISTINCT e.user_id) FROM events e 
                 JOIN event_types et ON e.event_type_id = et.id 
                 WHERE e.timestamp >= $1 AND e.timestamp <= $2 AND et.name = \
                     $3",
                    vec![Box::new(from), Box::new(to), Box::new(event_type)],
                )
            }
            else {
                (
                    "SELECT COUNT(DISTINCT user_id) FROM events WHERE \
                     timestamp >= $1 AND timestamp <= $2",
                    vec![Box::new(from), Box::new(to)],
                )
            };

        let stmt = client.prepare(query).await?;
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params
                .iter()
                .map(|p| {
                    p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)
                })
                .collect();

        let row = client.query_one(&stmt, &param_refs).await?;
        Ok(row.get(0))
    }

    #[instrument(skip(self))]
    pub async fn get_event_type_stats(
        &self, from: DateTime<Utc>, to: DateTime<Utc>,
        event_type: Option<String>,
    ) -> Result<Vec<(String, i64)>, EventError> {
        let client = self.db.get_analytics_client().await?;

        let (query, params): (&str, PgParamVec) =
            if let Some(event_type) = event_type {
                (
                    "SELECT et.name, COUNT(*) FROM events e 
                 JOIN event_types et ON e.event_type_id = et.id 
                 WHERE e.timestamp >= $1 AND e.timestamp <= $2 AND et.name = \
                     $3
                 GROUP BY et.name ORDER BY COUNT(*) DESC",
                    vec![Box::new(from), Box::new(to), Box::new(event_type)],
                )
            }
            else {
                (
                    "SELECT et.name, COUNT(*) FROM events e 
                 JOIN event_types et ON e.event_type_id = et.id 
                 WHERE e.timestamp >= $1 AND e.timestamp <= $2
                 GROUP BY et.name ORDER BY COUNT(*) DESC",
                    vec![Box::new(from), Box::new(to)],
                )
            };

        let stmt = client.prepare(query).await?;
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params
                .iter()
                .map(|p| {
                    p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)
                })
                .collect();

        let rows = client.query(&stmt, &param_refs).await?;
        let results = rows
            .into_iter()
            .map(|row| (row.get::<_, String>(0), row.get::<_, i64>(1)))
            .collect();

        Ok(results)
    }

    #[instrument(skip(self))]
    pub async fn get_top_pages(
        &self, from: DateTime<Utc>, to: DateTime<Utc>,
        event_type: Option<String>, limit: i64,
    ) -> Result<Vec<(String, i64)>, EventError> {
        let client = self.db.get_analytics_client().await?;

        let (query, params): (&str, PgParamVec) =
            if let Some(event_type) = event_type {
                (
                    "SELECT metadata->>'page' as page, COUNT(*) as count 
                 FROM events e 
                 JOIN event_types et ON e.event_type_id = et.id 
                 WHERE e.timestamp >= $1 AND e.timestamp <= $2 AND et.name = \
                     $3
                   AND metadata ? 'page' 
                 GROUP BY metadata->>'page' 
                 ORDER BY COUNT(*) DESC 
                 LIMIT $4",
                    vec![
                        Box::new(from),
                        Box::new(to),
                        Box::new(event_type),
                        Box::new(limit),
                    ],
                )
            }
            else {
                (
                    "SELECT metadata->>'page' as page, COUNT(*) as count 
                 FROM events e 
                 WHERE e.timestamp >= $1 AND e.timestamp <= $2
                   AND metadata ? 'page' 
                 GROUP BY metadata->>'page' 
                 ORDER BY COUNT(*) DESC 
                 LIMIT $3",
                    vec![Box::new(from), Box::new(to), Box::new(limit)],
                )
            };

        let stmt = client.prepare(query).await?;
        let param_refs: Vec<&(dyn tokio_postgres::types::ToSql + Sync)> =
            params
                .iter()
                .map(|p| {
                    p.as_ref() as &(dyn tokio_postgres::types::ToSql + Sync)
                })
                .collect();

        let rows = client.query(&stmt, &param_refs).await?;
        let results = rows
            .into_iter()
            .map(|row| {
                (
                    row.get::<_, Option<String>>(0)
                        .unwrap_or_else(|| "unknown".to_string()),
                    row.get::<_, i64>(1),
                )
            })
            .collect();

        Ok(results)
    }
}

#[async_trait]
impl GenericDao for EventDao {
    type CreateRequest = CreateEventCommand;
    type Error = EventError;
    type ID = i64;
    type Model = Event;
    type Response = EventResponse;
    type UpdateRequest = UpdateEventCommand;

    async fn find_by_id(
        &self, id: Self::ID,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_read_client().await?;
        let stmt = client
            .prepare(
                "SELECT e.id, e.user_id, e.event_type_id, e.timestamp, e.metadata, et.name \
                 FROM events e \
                 JOIN event_types et ON e.event_type_id = et.id \
                 WHERE e.id = $1",
            )
            .await?;
        let rows = client.query(&stmt, &[&id]).await?;

        let event_response = rows
            .first()
            .map(|row| self.map_row_to_response(row))
            .ok_or(EventError::NotFound { event_id: id })?;

        Ok(event_response)
    }

    async fn all(&self) -> Result<Vec<Self::Response>, Self::Error> {
        let client = self.db.get_read_client().await?;
        let stmt = client
            .prepare(
                "SELECT e.id, e.user_id, e.event_type_id, e.timestamp, e.metadata, et.name \
                 FROM events e \
                 JOIN event_types et ON e.event_type_id = et.id \
                 ORDER BY e.timestamp DESC",
            )
            .await?;
        let rows = client.query(&stmt, &[]).await?;

        let events = rows.iter().map(|row| self.map_row_to_response(row)).collect();

        Ok(events)
    }

    async fn create(
        &self, req: Self::CreateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;
        let timestamp = req.timestamp.unwrap_or_else(Utc::now);

        // Look up event type by name and create event in single query using
        // CTE - now includes event type name in result
        let stmt = client
            .prepare(
                "WITH event_type_lookup AS (
                     SELECT id as event_type_id, name as event_type_name FROM event_types WHERE name \
                 = $2
                 ),
                 event_insert AS (
                     INSERT INTO events (user_id, event_type_id, timestamp, \
                 metadata) 
                     SELECT $1, etl.event_type_id, $3, $4
                     FROM event_type_lookup etl
                     RETURNING id, user_id, event_type_id, timestamp, \
                 metadata
                 )
                 SELECT ei.id, ei.user_id, ei.event_type_id, ei.timestamp, \
                 ei.metadata,
                        CASE WHEN etl.event_type_id IS NULL THEN true ELSE \
                 false END as type_not_found,
                        COALESCE(etl.event_type_name, '') as event_type_name
                 FROM event_type_lookup etl
                 RIGHT JOIN event_insert ei ON true",
            )
            .await?;

        let rows = client
            .query(
                &stmt,
                &[&req.user_id, &req.event_type, &timestamp, &req.metadata],
            )
            .await?;

        if let Some(row) = rows.first() {
            let type_not_found: bool = row.get(5);
            if type_not_found {
                return Err(EventError::EventType(EventTypeError::NotFound));
            }

            let event_response = EventResponse {
                id: row.get(0),
                user_id: row.get(1),
                event_type_id: row.get(2),
                event_type: row.get(6), // event_type_name from query
                timestamp: row.get(3),
                metadata: row
                    .get::<_, Option<serde_json::Value>>(4)
                    .and_then(|json| serde_json::from_value(json).ok()),
            };
            Ok(event_response)
        }
        else {
            Err(EventError::Database(
                tokio_postgres::Error::__private_api_timeout(),
            ))
        }
    }

    async fn update(
        &self, id: Self::ID, req: Self::UpdateRequest,
    ) -> Result<Self::Response, Self::Error> {
        let client = self.db.get_client().await?;

        // Build dynamic update using CTE for validation and update in single
        // query
        match (&req.event_type_id, &req.metadata) {
            (Some(event_type_id), Some(metadata)) => {
                let stmt = client
                    .prepare(
                        "WITH validation AS (
                             SELECT CASE 
                                 WHEN NOT EXISTS(SELECT 1 FROM events WHERE \
                         id = $1) THEN 'not_found'::text
                                 ELSE 'ok'::text
                             END as status
                         ),
                         updated AS (
                             UPDATE events 
                             SET event_type_id = $2, metadata = $3
                             WHERE id = $1 
                             AND (SELECT status FROM validation) = 'ok'
                             RETURNING id, user_id, event_type_id, \
                         timestamp, metadata
                         )
                         SELECT u.id, u.user_id, u.event_type_id, \
                         u.timestamp, u.metadata, v.status
                         FROM validation v
                         LEFT JOIN updated u ON v.status = 'ok'",
                    )
                    .await?;

                let rows = client
                    .query(&stmt, &[&id, event_type_id, metadata])
                    .await?;

                if let Some(row) = rows.first() {
                    let status: String = row.get(5);
                    match status.as_str() {
                        "not_found" => {
                            Err(EventError::NotFound { event_id: id })
                        }
                        "ok" => {
                            let metadata_json: Option<serde_json::Value> =
                                row.get(4);
                            let metadata = metadata_json.and_then(|json| {
                                serde_json::from_value(json).ok()
                            });
                            // Need to get event type name for response
                            let event_type_name = client
                                .query_one("SELECT name FROM event_types WHERE id = $1", &[&row.get::<_, i32>(2)])
                                .await?
                                .get(0);
                            
                            let event_response = EventResponse {
                                id: row.get(0),
                                user_id: row.get(1),
                                event_type_id: row.get(2),
                                event_type: event_type_name,
                                timestamp: row.get(3),
                                metadata,
                            };
                            Ok(event_response)
                        }
                        _ => {
                            Err(EventError::Database(
                                tokio_postgres::Error::__private_api_timeout(
                                ),
                            ))
                        }
                    }
                }
                else {
                    Err(EventError::Database(
                        tokio_postgres::Error::__private_api_timeout(),
                    ))
                }
            }
            (Some(event_type_id), None) => {
                let stmt = client
                    .prepare(
                        "WITH updated AS (
                             UPDATE events SET event_type_id = $2 
                             WHERE id = $1 
                             RETURNING id, user_id, event_type_id, timestamp, metadata
                         )
                         SELECT u.id, u.user_id, u.event_type_id, u.timestamp, u.metadata, et.name
                         FROM updated u
                         JOIN event_types et ON u.event_type_id = et.id",
                    )
                    .await?;

                let rows = client.query(&stmt, &[&id, event_type_id]).await?;

                let event_response = rows
                    .first()
                    .map(|row| self.map_row_to_response(row))
                    .ok_or(EventError::NotFound { event_id: id })?;

                Ok(event_response)
            }
            (None, Some(metadata)) => {
                let stmt = client
                    .prepare(
                        "WITH updated AS (
                             UPDATE events SET metadata = $2 
                             WHERE id = $1 
                             RETURNING id, user_id, event_type_id, timestamp, metadata
                         )
                         SELECT u.id, u.user_id, u.event_type_id, u.timestamp, u.metadata, et.name
                         FROM updated u
                         JOIN event_types et ON u.event_type_id = et.id",
                    )
                    .await?;

                let rows = client.query(&stmt, &[&id, metadata]).await?;

                let event_response = rows
                    .first()
                    .map(|row| self.map_row_to_response(row))
                    .ok_or(EventError::NotFound { event_id: id })?;

                Ok(event_response)
            }
            (None, None) => {
                // No updates, just return the existing event
                self.find_by_id(id).await
            }
        }
    }

    async fn delete(&self, id: Self::ID) -> Result<(), Self::Error> {
        let client = self.db.get_client().await?;

        // Use RETURNING clause to check if row existed
        let stmt = client
            .prepare("DELETE FROM events WHERE id = $1 RETURNING id")
            .await?;
        let affected = client.execute(&stmt, &[&id]).await?;

        if affected == 0 {
            return Err(EventError::NotFound { event_id: id });
        }

        Ok(())
    }

    fn map_row(&self, row: &tokio_postgres::Row) -> Self::Model {
        let metadata_json: Option<serde_json::Value> = row.get(4);
        let metadata =
            metadata_json.and_then(|json| serde_json::from_value(json).ok());

        Event {
            id: row.get(0),
            user_id: row.get(1),
            event_type_id: row.get(2),
            timestamp: row.get(3),
            metadata,
        }
    }


    async fn count(&self) -> Result<i64, Self::Error> {
        let client = self.db.get_read_client().await?;
        let stmt = client.prepare("SELECT COUNT(*) FROM events").await?;
        let rows = client.query(&stmt, &[]).await?;

        let count: i64 = rows.first().map(|row| row.get(0)).unwrap_or(0);

        Ok(count)
    }
}

impl EventDao {
    #[instrument(skip_all)]
    pub async fn find_with_filters(
        &self, user_id: Option<i64>, event_type_id: Option<i32>,
        limit: Option<u64>, offset: Option<u64>,
    ) -> Result<Vec<EventResponse>, EventError> {
        let client = self.db.get_read_client().await?;

        // Build query dynamically based on provided filters
        let mut query = String::from(
            "SELECT e.id, e.user_id, e.event_type_id, e.timestamp, e.metadata, et.name \
             FROM events e \
             JOIN event_types et ON e.event_type_id = et.id",
        );
        let mut where_clauses = Vec::new();
        let mut params: PgParamVec = Vec::new();
        let mut param_count = 0;

        // Add WHERE filters
        if let Some(uid) = user_id {
            param_count += 1;
            where_clauses.push(format!("e.user_id = ${param_count}"));
            params.push(Box::new(uid));
        }

        if let Some(etid) = event_type_id {
            param_count += 1;
            where_clauses.push(format!("e.event_type_id = ${param_count}"));
            params.push(Box::new(etid));
        }

        if !where_clauses.is_empty() {
            query.push_str(" WHERE ");
            query.push_str(&where_clauses.join(" AND "));
        }

        // Add ORDER BY
        query.push_str(" ORDER BY e.timestamp DESC");

        // Add LIMIT and OFFSET
        if let Some(l) = limit {
            param_count += 1;
            query.push_str(&format!(" LIMIT ${param_count}"));
            params.push(Box::new(l as i64));
        }

        if let Some(o) = offset {
            param_count += 1;
            query.push_str(&format!(" OFFSET ${param_count}"));
            params.push(Box::new(o as i64));
        }

        let stmt = client.prepare(&query).await?;

        let param_refs: Vec<&PgParam> =
            params.iter().map(|p| p.as_ref() as &PgParam).collect();

        let rows = client.query(&stmt, &param_refs).await?;

        let events = rows.iter().map(|row| self.map_row_to_response(row)).collect();

        Ok(events)
    }

    #[instrument(skip_all)]
    pub async fn delete_before_timestamp(
        &self, before: DateTime<Utc>,
    ) -> Result<u64, EventError> {
        let client = self.db.get_client().await?;
        let stmt = client
            .prepare("DELETE FROM events WHERE timestamp < $1")
            .await?;
        let affected = client.execute(&stmt, &[&before]).await?;
        Ok(affected)
    }

    #[instrument(skip_all)]
    pub async fn find_by_user_id(
        &self, user_id: i64, limit: Option<u64>,
    ) -> Result<Vec<EventResponse>, EventError> {
        let client = self.db.get_read_client().await?;

        let (sql, params): (String, Vec<i64>) = match limit {
            Some(l) => {
                (
                    "SELECT e.id, e.user_id, e.event_type_id, e.timestamp, e.metadata, et.name 
                 FROM events e 
                 JOIN event_types et ON e.event_type_id = et.id 
                 WHERE e.user_id = $1 
                 ORDER BY e.timestamp DESC LIMIT $2"
                        .to_string(),
                    vec![l as i64],
                )
            }
            None => {
                (
                    "SELECT e.id, e.user_id, e.event_type_id, e.timestamp, e.metadata, et.name 
                 FROM events e 
                 JOIN event_types et ON e.event_type_id = et.id 
                 WHERE e.user_id = $1 
                 ORDER BY e.timestamp DESC"
                        .to_string(),
                    vec![],
                )
            }
        };

        let stmt = client.prepare(&sql).await?;

        let param_refs: Vec<&PgParam> = std::iter::once(&user_id as &PgParam)
            .chain(params.iter().map(|p| p as &PgParam))
            .collect();

        let rows = client.query(&stmt, &param_refs).await?;
        let events = rows.iter().map(|row| self.map_row_to_response(row)).collect();

        Ok(events)
    }

    #[instrument(skip_all)]
    pub async fn find_with_pagination(
        &self, limit: Option<u64>, offset: Option<u64>,
    ) -> Result<Vec<Event>, EventError> {
        let client = self.db.get_read_client().await?;

        let (sql, params): (String, Vec<i64>) = match (limit, offset) {
            (Some(l), Some(o)) => {
                (
                    "SELECT id, user_id, event_type_id, timestamp, metadata \
                     FROM events ORDER BY timestamp DESC LIMIT $1 OFFSET $2"
                        .to_string(),
                    vec![l as i64, o as i64],
                )
            }
            (Some(l), None) => {
                (
                    "SELECT id, user_id, event_type_id, timestamp, metadata \
                     FROM events ORDER BY timestamp DESC LIMIT $1"
                        .to_string(),
                    vec![l as i64],
                )
            }
            (None, Some(o)) => {
                (
                    "SELECT id, user_id, event_type_id, timestamp, metadata \
                     FROM events ORDER BY timestamp DESC OFFSET $1"
                        .to_string(),
                    vec![o as i64],
                )
            }
            (None, None) => {
                (
                    "SELECT id, user_id, event_type_id, timestamp, metadata \
                     FROM events ORDER BY timestamp DESC"
                        .to_string(),
                    vec![],
                )
            }
        };

        let stmt = client.prepare(&sql).await?;

        let param_refs: Vec<&PgParam> =
            params.iter().map(|p| p as &PgParam).collect();

        let rows = client.query(&stmt, &param_refs).await?;
        let events = rows.iter().map(|row| self.map_row(row)).collect();

        Ok(events)
    }

    #[instrument(skip_all)]
    pub async fn find_with_cursor(
        &self, cursor: Option<DateTime<Utc>>, limit: u64,
    ) -> Result<(Vec<Event>, Option<DateTime<Utc>>), EventError> {
        let client = self.db.get_read_client().await?;
        let limit = limit.min(1000);
        let limit_plus_one = limit as i64 + 1;

        let (_sql, rows) = match cursor {
            Some(cursor_timestamp) => {
                let sql = "SELECT id, user_id, event_type_id, timestamp, \
                           metadata FROM events 
                          WHERE timestamp < $1 
                          ORDER BY timestamp DESC 
                          LIMIT $2";
                let stmt = client.prepare(sql).await?;
                let rows = client
                    .query(&stmt, &[&cursor_timestamp, &limit_plus_one])
                    .await?;
                (sql, rows)
            }
            None => {
                let sql = "SELECT id, user_id, event_type_id, timestamp, \
                           metadata FROM events 
                          ORDER BY timestamp DESC 
                          LIMIT $1";
                let stmt = client.prepare(sql).await?;
                let rows = client.query(&stmt, &[&limit_plus_one]).await?;
                (sql, rows)
            }
        };

        let events: Vec<Event> = rows
            .iter()
            .take(limit as usize)
            .map(|row| self.map_row(row))
            .collect();

        let next_cursor = if rows.len() > limit as usize {
            events.last().map(|e| e.timestamp)
        }
        else {
            None
        };

        Ok((events, next_cursor))
    }

    #[instrument(skip_all)]
    pub async fn find_by_user_with_cursor(
        &self, user_id: i64, cursor: Option<DateTime<Utc>>, limit: u64,
    ) -> Result<(Vec<Event>, Option<DateTime<Utc>>), EventError> {
        let client = self.db.get_read_client().await?;
        let limit = limit.min(1000);
        let limit_plus_one = limit as i64 + 1;

        let (_sql, rows) = match cursor {
            Some(cursor_timestamp) => {
                let sql = "SELECT id, user_id, event_type_id, timestamp, \
                           metadata FROM events 
                          WHERE user_id = $1 AND timestamp < $2 
                          ORDER BY timestamp DESC 
                          LIMIT $3";
                let stmt = client.prepare(sql).await?;
                let rows = client
                    .query(
                        &stmt,
                        &[&user_id, &cursor_timestamp, &limit_plus_one],
                    )
                    .await?;
                (sql, rows)
            }
            None => {
                let sql = "SELECT id, user_id, event_type_id, timestamp, \
                           metadata FROM events 
                          WHERE user_id = $1 
                          ORDER BY timestamp DESC 
                          LIMIT $2";
                let stmt = client.prepare(sql).await?;
                let rows =
                    client.query(&stmt, &[&user_id, &limit_plus_one]).await?;
                (sql, rows)
            }
        };

        let events: Vec<Event> = rows
            .iter()
            .take(limit as usize)
            .map(|row| self.map_row(row))
            .collect();

        let next_cursor = if rows.len() > limit as usize {
            events.last().map(|e| e.timestamp)
        }
        else {
            None
        };

        Ok((events, next_cursor))
    }
}
