use anyhow::Result;
use sql_connection::SqlConnect;
use uuid::Uuid;

use crate::postgres::TestPostgresContainer;

pub async fn create_test_event_type(
    container: &TestPostgresContainer,
) -> Result<i32> {
    let sqlx_pool = container.connection.get_postgres_connection_pool();
    let row = sqlx::query_as::<_, (i32,)>(
        "INSERT INTO event_types (name) VALUES ('test_event') ON CONFLICT \
         (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .fetch_one(sqlx_pool)
    .await?;
    Ok(row.0)
}

pub async fn create_test_event_types(
    container: &TestPostgresContainer,
) -> Result<(i32, i32)> {
    let sqlx_pool = container.connection.get_postgres_connection_pool();
    let login_row = sqlx::query_as::<_, (i32,)>(
        "INSERT INTO event_types (name) VALUES ('login_event') ON CONFLICT \
         (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .fetch_one(sqlx_pool)
    .await?;
    let logout_row = sqlx::query_as::<_, (i32,)>(
        "INSERT INTO event_types (name) VALUES ('logout_event') ON CONFLICT \
         (name) DO UPDATE SET name = EXCLUDED.name RETURNING id",
    )
    .fetch_one(sqlx_pool)
    .await?;
    Ok((login_row.0, logout_row.0))
}

pub async fn create_test_user(
    container: &TestPostgresContainer,
) -> Result<Uuid> {
    let user_id = Uuid::now_v7();
    let query = format!(
        "INSERT INTO users (id, name, created_at) VALUES ('{}', 'Test \
         User', NOW())",
        user_id
    );
    container.execute_sql(&query).await?;
    Ok(user_id)
}

pub async fn create_test_user_with_name(
    container: &TestPostgresContainer, name: &str,
) -> Result<Uuid> {
    let user_id = Uuid::now_v7();
    let query = format!(
        "INSERT INTO users (id, name, created_at) VALUES ('{}', '{}', NOW())",
        user_id, name
    );
    container.execute_sql(&query).await?;
    Ok(user_id)
}

pub async fn create_test_users(
    container: &TestPostgresContainer,
) -> Result<(Uuid, Uuid)> {
    let user1_id = create_test_user_with_name(container, "Alice").await?;
    let user2_id = create_test_user_with_name(container, "Bob").await?;
    Ok((user1_id, user2_id))
}

pub async fn create_test_event(
    container: &TestPostgresContainer, user_id: Uuid, event_type_id: i32,
    metadata: Option<&str>,
) -> Result<Uuid> {
    let event_id = Uuid::now_v7();
    let metadata_value = metadata.unwrap_or("{}");
    let query = format!(
        "INSERT INTO events (id, user_id, event_type_id, metadata, \
         timestamp) VALUES ('{}', '{}', {}, '{}', NOW())",
        event_id, user_id, event_type_id, metadata_value
    );
    container.execute_sql(&query).await?;
    Ok(event_id)
}

pub async fn clean_test_data(
    container: &TestPostgresContainer,
) -> Result<()> {
    container.execute_sql("DELETE FROM events").await?;
    container.execute_sql("DELETE FROM event_types").await?;
    container.execute_sql("DELETE FROM users").await?;
    Ok(())
}

pub fn create_sql_connect(container: &TestPostgresContainer) -> SqlConnect {
    SqlConnect::new(container.connection.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::postgres::TestPostgresContainer;

    #[tokio::test]
    async fn test_create_test_event_type() {
        let container = TestPostgresContainer::new().await.unwrap();

        // Clean any existing data
        let _ = clean_test_data(&container).await;

        let event_type_id = create_test_event_type(&container).await.unwrap();
        assert!(event_type_id > 0);

        // Clean up after test
        let _ = clean_test_data(&container).await;
    }

    #[tokio::test]
    async fn test_create_test_user() {
        let container = TestPostgresContainer::new().await.unwrap();

        // Clean any existing data
        let _ = clean_test_data(&container).await;

        let user_id = create_test_user(&container).await.unwrap();
        let result = container
            .execute_sql(&format!(
                "SELECT 1 FROM users WHERE id = '{}'",
                user_id
            ))
            .await;
        assert!(result.is_ok());

        // Clean up after test
        let _ = clean_test_data(&container).await;
    }

    #[tokio::test]
    async fn test_create_test_event() {
        let container = TestPostgresContainer::new().await.unwrap();

        // Clean any existing data
        let _ = clean_test_data(&container).await;

        let event_type_id = create_test_event_type(&container).await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();
        let event_id = create_test_event(
            &container,
            user_id,
            event_type_id,
            Some(r#"{"test": "data"}"#),
        )
        .await
        .unwrap();

        let result = container
            .execute_sql(&format!(
                "SELECT 1 FROM events WHERE id = '{}'",
                event_id
            ))
            .await;
        assert!(result.is_ok());

        // Clean up after test
        let _ = clean_test_data(&container).await;
    }
}
