use anyhow::Result;
use sql_connection::SqlConnect;
use uuid::Uuid;

use crate::postgres::TestPostgresContainer;

/// Create a test event type with id=1 and name='test_event'
pub async fn create_test_event_type(
    container: &TestPostgresContainer,
) -> Result<i32> {
    container
        .execute_sql(
            "INSERT INTO event_types (id, name) VALUES (1, 'test_event')",
        )
        .await?;
    Ok(1)
}

/// Create two test event types and return their IDs
pub async fn create_test_event_types(
    container: &TestPostgresContainer,
) -> Result<(i32, i32)> {
    container
        .execute_sql(
            "INSERT INTO event_types (id, name) VALUES (1, 'login_event')",
        )
        .await?;
    container
        .execute_sql(
            "INSERT INTO event_types (id, name) VALUES (2, 'logout_event')",
        )
        .await?;
    Ok((1, 2))
}

/// Create a test user with generated UUID and return the user ID
pub async fn create_test_user(
    container: &TestPostgresContainer,
) -> Result<Uuid> {
    let user_id = Uuid::now_v7();
    let query = format!(
        "INSERT INTO users (id, name, email, created_at, updated_at) VALUES \
         ('{}', 'Test User', 'test@example.com', NOW(), NOW())",
        user_id
    );
    container.execute_sql(&query).await?;
    Ok(user_id)
}

/// Create a test user with a specific name
pub async fn create_test_user_with_name(
    container: &TestPostgresContainer, name: &str,
) -> Result<Uuid> {
    let user_id = Uuid::now_v7();
    let query = format!(
        "INSERT INTO users (id, name, email, created_at, updated_at) VALUES \
         ('{}', '{}', '{}@example.com', NOW(), NOW())",
        user_id,
        name,
        name.to_lowercase().replace(" ", "")
    );
    container.execute_sql(&query).await?;
    Ok(user_id)
}

/// Create two test users and return their UUIDs
pub async fn create_test_users(
    container: &TestPostgresContainer,
) -> Result<(Uuid, Uuid)> {
    let user1_id = create_test_user_with_name(container, "Alice").await?;
    let user2_id = create_test_user_with_name(container, "Bob").await?;
    Ok((user1_id, user2_id))
}

/// Create a test event and return its UUID
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

/// Clean all test data from the database (useful for cleanup between tests if
/// needed)
pub async fn clean_test_data(
    container: &TestPostgresContainer,
) -> Result<()> {
    // Clean in dependency order
    container.execute_sql("DELETE FROM events").await?;
    container.execute_sql("DELETE FROM event_types").await?;
    container.execute_sql("DELETE FROM users").await?;
    Ok(())
}

/// Create a SQL connection from a test container for use with DAOs and
/// handlers
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
        let event_type_id = create_test_event_type(&container).await.unwrap();
        assert_eq!(event_type_id, 1);
    }

    #[tokio::test]
    async fn test_create_test_user() {
        let container = TestPostgresContainer::new().await.unwrap();
        let user_id = create_test_user(&container).await.unwrap();

        // Verify user was created
        let result = container
            .execute_sql(&format!(
                "SELECT 1 FROM users WHERE id = '{}'",
                user_id
            ))
            .await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_test_event() {
        let container = TestPostgresContainer::new().await.unwrap();

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

        // Verify event was created
        let result = container
            .execute_sql(&format!(
                "SELECT 1 FROM events WHERE id = '{}'",
                event_id
            ))
            .await;
        assert!(result.is_ok());
    }
}
