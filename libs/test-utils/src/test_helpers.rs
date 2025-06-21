use anyhow::Result;
use serde_json::Value;
use sql_connection::SqlConnect;
use uuid::Uuid;

use crate::TestPostgresContainer;

pub async fn create_test_event_type(
    container: &TestPostgresContainer,
) -> Result<i32> {
    create_test_event_type_modern(container).await
}

pub async fn create_test_event_type_with_name(
    container: &TestPostgresContainer, name: &str,
) -> Result<i32> {
    create_test_event_type_with_name_modern(container, name).await
}

pub async fn create_test_event_types(
    container: &TestPostgresContainer,
) -> Result<(i32, i32)> {
    create_test_event_types_modern(container).await
}

pub async fn create_unique_event_types(
    container: &TestPostgresContainer, suffix: &str,
) -> Result<(i32, i32)> {
    create_unique_event_types_modern(container, suffix).await
}

pub async fn create_test_user(
    container: &TestPostgresContainer,
) -> Result<Uuid> {
    create_test_user_modern(container).await
}

pub async fn create_test_user_with_name(
    container: &TestPostgresContainer, name: &str,
) -> Result<Uuid> {
    create_test_user_with_name_modern(container, name).await
}

pub async fn create_test_users(
    container: &TestPostgresContainer,
) -> Result<(Uuid, Uuid)> {
    create_test_users_modern(container).await
}

pub async fn create_test_event(
    container: &TestPostgresContainer, user_id: Uuid, event_type_id: i32,
    metadata: Option<&str>,
) -> Result<Uuid> {
    create_test_event_modern(container, user_id, event_type_id, metadata)
        .await
}

pub async fn clean_test_data(
    container: &TestPostgresContainer,
) -> Result<()> {
    clean_test_data_modern(container).await
}

pub fn create_sql_connect(container: &TestPostgresContainer) -> SqlConnect {
    create_sql_connect_modern(container)
}

// Modern container helper functions
pub async fn create_test_event_type_modern(
    container: &TestPostgresContainer,
) -> Result<i32> {
    create_test_event_type_with_name_modern(container, "test_event").await
}

pub async fn create_test_event_type_with_name_modern(
    container: &TestPostgresContainer, name: &str,
) -> Result<i32> {
    let client = container.pool.get().await?;
    let row = client
        .query_one(
            "INSERT INTO event_types (name) VALUES ($1) RETURNING id",
            &[&name],
        )
        .await?;
    Ok(row.get(0))
}

pub async fn create_test_event_types_modern(
    container: &TestPostgresContainer,
) -> Result<(i32, i32)> {
    let client = container.pool.get().await?;
    let login_row = client
        .query_one(
            "INSERT INTO event_types (name) VALUES ('login_event') \
             RETURNING id",
            &[],
        )
        .await?;
    let logout_row = client
        .query_one(
            "INSERT INTO event_types (name) VALUES ('logout_event') \
             RETURNING id",
            &[],
        )
        .await?;
    Ok((login_row.get(0), logout_row.get(0)))
}

pub async fn create_unique_event_types_modern(
    container: &TestPostgresContainer, suffix: &str,
) -> Result<(i32, i32)> {
    let client = container.pool.get().await?;
    let login_name = format!("login_event_{}", suffix);
    let logout_name = format!("logout_event_{}", suffix);

    let login_row = client
        .query_one(
            "INSERT INTO event_types (name) VALUES ($1) RETURNING id",
            &[&login_name],
        )
        .await?;
    let logout_row = client
        .query_one(
            "INSERT INTO event_types (name) VALUES ($1) RETURNING id",
            &[&logout_name],
        )
        .await?;
    Ok((login_row.get(0), logout_row.get(0)))
}

pub async fn create_test_user_modern(
    container: &TestPostgresContainer,
) -> Result<Uuid> {
    let user_id = Uuid::now_v7();
    let client = container.pool.get().await?;
    client
        .execute(
            "INSERT INTO users (id, name, created_at) VALUES ($1, 'Test \
             User', NOW())",
            &[&user_id],
        )
        .await?;
    Ok(user_id)
}

pub async fn create_test_user_with_name_modern(
    container: &TestPostgresContainer, name: &str,
) -> Result<Uuid> {
    let user_id = Uuid::now_v7();
    let client = container.pool.get().await?;
    client
        .execute(
            "INSERT INTO users (id, name, created_at) VALUES ($1, $2, NOW())",
            &[&user_id, &name],
        )
        .await?;
    Ok(user_id)
}

pub async fn create_test_users_modern(
    container: &TestPostgresContainer,
) -> Result<(Uuid, Uuid)> {
    let user1_id =
        create_test_user_with_name_modern(container, "Alice").await?;
    let user2_id =
        create_test_user_with_name_modern(container, "Bob").await?;
    Ok((user1_id, user2_id))
}

pub async fn create_test_event_modern(
    container: &TestPostgresContainer, user_id: Uuid, event_type_id: i32,
    metadata: Option<&str>,
) -> Result<Uuid> {
    let event_id = Uuid::now_v7();
    let client = container.pool.get().await?;

    let metadata_json = metadata.map(Value::from);

    client
        .execute(
            "INSERT INTO events (id, user_id, event_type_id, metadata, \
             timestamp) VALUES ($1, $2, $3, $4, NOW())",
            &[&event_id, &user_id, &event_type_id, &metadata_json],
        )
        .await?;
    Ok(event_id)
}

pub async fn clean_test_data_modern(
    container: &TestPostgresContainer,
) -> Result<()> {
    container.execute_sql("DELETE FROM events").await?;
    container.execute_sql("DELETE FROM users").await?;
    container.execute_sql("DELETE FROM event_types").await?;
    Ok(())
}

pub fn create_sql_connect_modern(
    container: &TestPostgresContainer,
) -> SqlConnect {
    SqlConnect::new(container.pool.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TestPostgresContainer;

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
