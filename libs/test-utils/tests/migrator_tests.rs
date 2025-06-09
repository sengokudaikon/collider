use anyhow::Result;
use test_utils::TestPostgresInstance;

#[tokio::test]
async fn test_migrator_up_integration() -> Result<()> {
    let postgres = TestPostgresInstance::new_with_unique_db().await?;
    let migrator = postgres.get_migrator().await?;

    migrator.reset_all().await?;

    let applied = migrator.list_applied_migrations().await?;
    assert!(applied.is_empty());

    migrator.run_all_migrations().await?;

    let applied = migrator.list_applied_migrations().await?;
    assert!(!applied.is_empty());
    assert!(applied.contains(&"001_create_users".to_string()));
    assert!(applied.contains(&"002_create_event_types".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_migrator_down_integration() -> Result<()> {
    let postgres = TestPostgresInstance::new_with_unique_db().await?;
    let migrator = postgres.get_migrator().await?;

    let applied_before = migrator.list_applied_migrations().await?;
    let count_before = applied_before.len();
    assert!(count_before > 0);

    let to_rollback = vec![applied_before.last().unwrap().as_str()];
    migrator.run_down_migrations(&to_rollback).await?;

    let applied_after = migrator.list_applied_migrations().await?;
    assert_eq!(applied_after.len(), count_before - 1);

    Ok(())
}

#[tokio::test]
async fn test_migrator_reset_integration() -> Result<()> {
    let postgres = TestPostgresInstance::new_with_unique_db().await?;
    let migrator = postgres.get_migrator().await?;

    let applied_before = migrator.list_applied_migrations().await?;
    assert!(!applied_before.is_empty());

    migrator.reset_all().await?;

    let applied_after = migrator.list_applied_migrations().await?;
    assert!(applied_after.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_migrator_status_integration() -> Result<()> {
    let postgres = TestPostgresInstance::new_with_unique_db().await?;
    let migrator = postgres.get_migrator().await?;

    migrator.reset_all().await?;

    let applied = migrator.list_applied_migrations().await?;
    assert!(applied.is_empty());

    migrator
        .run_migration(
            "test_migration",
            "CREATE TABLE test_table (id SERIAL PRIMARY KEY);",
        )
        .await?;

    let applied = migrator.list_applied_migrations().await?;
    assert_eq!(applied.len(), 1);
    assert!(applied.contains(&"test_migration".to_string()));

    Ok(())
}

#[tokio::test]
async fn test_migrator_idempotent() -> Result<()> {
    let postgres = TestPostgresInstance::new_with_unique_db().await?;
    let migrator = postgres.get_migrator().await?;

    migrator.reset_all().await?;

    migrator.run_all_migrations().await?;
    let applied_first = migrator.list_applied_migrations().await?;

    migrator.run_all_migrations().await?;
    let applied_second = migrator.list_applied_migrations().await?;

    assert_eq!(applied_first, applied_second);

    Ok(())
}

#[tokio::test]
async fn test_migrator_single_migration() -> Result<()> {
    let postgres = TestPostgresInstance::new_with_unique_db().await?;
    let migrator = postgres.get_migrator().await?;

    migrator.reset_all().await?;

    let migration_sql = r#"
        CREATE TABLE custom_test (
            id SERIAL PRIMARY KEY,
            name VARCHAR(255) NOT NULL,
            created_at TIMESTAMP DEFAULT NOW()
        );
    "#;

    migrator.run_migration("test_custom", migration_sql).await?;

    let applied = migrator.list_applied_migrations().await?;
    assert_eq!(applied.len(), 1);
    assert!(applied.contains(&"test_custom".to_string()));

    postgres
        .execute_sql("INSERT INTO custom_test (name) VALUES ('test')")
        .await?;

    Ok(())
}
