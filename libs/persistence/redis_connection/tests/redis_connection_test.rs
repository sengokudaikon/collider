use std::time::Duration;

use deadpool_redis::redis::AsyncCommands;
use redis_connection::{
    config::{DbConnectConfig, RedisDbConfig},
    connection::RedisConnectionManager,
    core::command::{IntoRedisCommands, RedisCommands, RedisCommandsExt},
};
use test_utils::redis::TestRedisContainer;

async fn setup_test_redis()
-> anyhow::Result<(TestRedisContainer, RedisConnectionManager)> {
    let container = TestRedisContainer::new().await?;

    // Clean any existing test data to ensure test isolation
    container.flush_db().await?;

    let manager = RedisConnectionManager::new(container.pool.clone());
    Ok((container, manager))
}

// Helper function to get test-prefixed key
fn test_key(container: &TestRedisContainer, key: &str) -> String {
    container.test_key(key)
}

#[tokio::test]
async fn test_redis_db_config_defaults() {
    let config = RedisDbConfig {
        host: "127.0.0.1".to_string(),
        port: 6379,
        db: 0,
    };

    assert_eq!(config.host(), "127.0.0.1");
    assert_eq!(config.port(), 6379);
    assert_eq!(config.db(), 0);
}

#[tokio::test]
async fn test_redis_db_config_from_json() {
    let json = r#"{
        "host": "redis.example.com",
        "port": 6380,
        "db": 1
    }"#;

    let config: RedisDbConfig = serde_json::from_str(json).unwrap();

    assert_eq!(config.host(), "redis.example.com");
    assert_eq!(config.port(), 6380);
    assert_eq!(config.db(), 1);
}

#[tokio::test]
async fn test_redis_db_config_defaults_from_empty_json() {
    let json = r#"{}"#;
    let config: RedisDbConfig = serde_json::from_str(json).unwrap();

    assert_eq!(config.host(), "127.0.0.1");
    assert_eq!(config.port(), 6379);
    assert_eq!(config.db(), 0);
}

#[tokio::test]
async fn test_redis_db_config_trait_implementation() {
    let config = RedisDbConfig {
        host: "test.redis.host".to_string(),
        port: 6380,
        db: 1,
    };

    assert_eq!(config.host(), "test.redis.host");
    assert_eq!(config.port(), 6380);
    assert_eq!(config.db(), 1);
    assert_eq!(config.password(), None);
}

#[tokio::test]
async fn test_redis_connection_manager() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    let mut conn = manager.get_connection().await.unwrap();

    let pong: String = conn.ping().await.unwrap();
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_redis_connection_manager_mut() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    let mut conn = manager.get_connection().await.unwrap();

    let _: () = conn.set("test_key", "test_value").await.unwrap();
    let value: String = conn.get("test_key").await.unwrap();
    assert_eq!(value, "test_value");
}

#[tokio::test]
async fn test_redis_basic_operations() {
    let (container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let string_key = test_key(&container, "string_key");
    let int_key = test_key(&container, "int_key");
    let nonexistent_key = test_key(&container, "nonexistent_key");

    let _: () = conn.set(&string_key, "hello world").await.unwrap();
    let value: String = conn.get(&string_key).await.unwrap();
    assert_eq!(value, "hello world");

    let _: () = conn.set(&int_key, 42i32).await.unwrap();
    let int_value: i32 = conn.get(&int_key).await.unwrap();
    assert_eq!(int_value, 42);

    let exists: bool = conn.exists(&string_key).await.unwrap();
    assert!(exists);

    let not_exists: bool = conn.exists(&nonexistent_key).await.unwrap();
    assert!(!not_exists);

    let _: () = conn.del(&string_key).await.unwrap();
    let exists_after_del: bool = conn.exists(&string_key).await.unwrap();
    assert!(!exists_after_del);
}

#[tokio::test]
async fn test_redis_hash_operations() {
    let (container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let hash_key = test_key(&container, "hash_key");

    let _: () = conn.hset(&hash_key, "field1", "value1").await.unwrap();
    let _: () = conn.hset(&hash_key, "field2", "value2").await.unwrap();

    let value1: String = conn.hget(&hash_key, "field1").await.unwrap();
    assert_eq!(value1, "value1");

    let value2: String = conn.hget(&hash_key, "field2").await.unwrap();
    assert_eq!(value2, "value2");

    let field_exists: bool = conn.hexists(&hash_key, "field1").await.unwrap();
    assert!(field_exists);

    let field_not_exists: bool =
        conn.hexists(&hash_key, "nonexistent_field").await.unwrap();
    assert!(!field_not_exists);

    let all_fields: std::collections::HashMap<String, String> =
        conn.hgetall(&hash_key).await.unwrap();
    assert_eq!(all_fields.len(), 2);
    assert_eq!(all_fields.get("field1"), Some(&"value1".to_string()));
    assert_eq!(all_fields.get("field2"), Some(&"value2".to_string()));

    let _: () = conn.hdel(&hash_key, "field1").await.unwrap();
    let field_exists_after_del: bool =
        conn.hexists(&hash_key, "field1").await.unwrap();
    assert!(!field_exists_after_del);
}

#[tokio::test]
async fn test_redis_list_operations() {
    let (container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let list_key = test_key(&container, "list_key");

    let _: () = conn.lpush(&list_key, "item1").await.unwrap();
    let _: () = conn.lpush(&list_key, "item2").await.unwrap();
    let _: () = conn.rpush(&list_key, "item3").await.unwrap();

    let length: i32 = conn.llen(&list_key).await.unwrap();
    assert_eq!(length, 3);

    let items: Vec<String> = conn.lrange(&list_key, 0, -1).await.unwrap();
    assert_eq!(items, vec!["item2", "item1", "item3"]);

    let popped: String = conn.lpop(&list_key, None).await.unwrap();
    assert_eq!(popped, "item2");

    let length_after_pop: i32 = conn.llen(&list_key).await.unwrap();
    assert_eq!(length_after_pop, 2);
}

#[tokio::test]
async fn test_redis_set_operations() {
    let (container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let set_key = test_key(&container, "set_key");

    let _: () = conn.sadd(&set_key, "member1").await.unwrap();
    let _: () = conn.sadd(&set_key, "member2").await.unwrap();
    let _: () = conn.sadd(&set_key, "member3").await.unwrap();

    let is_member: bool = conn.sismember(&set_key, "member1").await.unwrap();
    assert!(is_member);

    let not_member: bool =
        conn.sismember(&set_key, "nonexistent").await.unwrap();
    assert!(!not_member);

    let cardinality: i32 = conn.scard(&set_key).await.unwrap();
    assert_eq!(cardinality, 3);

    let members: std::collections::HashSet<String> =
        conn.smembers(&set_key).await.unwrap();
    assert_eq!(members.len(), 3);
    assert!(members.contains("member1"));
    assert!(members.contains("member2"));
    assert!(members.contains("member3"));

    let _: () = conn.srem(&set_key, "member1").await.unwrap();
    let cardinality_after_remove: i32 = conn.scard(&set_key).await.unwrap();
    assert_eq!(cardinality_after_remove, 2);
}

#[tokio::test]
async fn test_redis_expire_operations() {
    let (container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let temp_key = test_key(&container, "temp_key");

    let _: () = conn.set(&temp_key, "temporary_value").await.unwrap();
    let _: () = conn.expire(&temp_key, 60).await.unwrap();

    let ttl: i32 = conn.ttl(&temp_key).await.unwrap();
    assert!(ttl > 0 && ttl <= 60);

    let _: () = conn.persist(&temp_key).await.unwrap();
    let ttl_after_persist: i32 = conn.ttl(&temp_key).await.unwrap();
    assert_eq!(ttl_after_persist, -1);
}

#[tokio::test]
async fn test_cache_key_pattern_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let _: () = conn.set("pattern:1", "value1").await.unwrap();
    let _: () = conn.set("pattern:2", "value2").await.unwrap();
    let _: () = conn.set("other:key", "value3").await.unwrap();

    let keys: Vec<String> = conn.keys("pattern:*").await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"pattern:1".to_string()));
    assert!(keys.contains(&"pattern:2".to_string()));

    let key_type: String = conn.key_type("pattern:1").await.unwrap();
    assert_eq!(key_type, "string");

    let _: () = conn.rename("pattern:1", "renamed:1").await.unwrap();
    let exists_old: bool = conn.exists("pattern:1").await.unwrap();
    let exists_new: bool = conn.exists("renamed:1").await.unwrap();
    assert!(!exists_old);
    assert!(exists_new);
}

#[tokio::test]
async fn test_concurrent_redis_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    let tasks = (0..10).map(|i| {
        let manager = manager.clone();
        tokio::spawn(async move {
            let mut conn = manager.get_connection().await.unwrap();
            let key = format!("concurrent:key:{}", i);
            let value = format!("value_{}", i);

            let _: () = conn.set(&key, &value).await.unwrap();
            let retrieved: String = conn.get(&key).await.unwrap();
            assert_eq!(retrieved, value);
        })
    });

    for task in tasks {
        task.await.unwrap();
    }

    let mut conn = manager.get_connection().await.unwrap();
    let keys: Vec<String> = conn.keys("concurrent:key:*").await.unwrap();
    assert_eq!(keys.len(), 10);
}

#[tokio::test]
async fn test_redis_connection_manager_static() {
    let (_container, _manager) = setup_test_redis().await.unwrap();

    let another_container = TestRedisContainer::new().await.unwrap();
    RedisConnectionManager::init_static(another_container.pool);

    let static_manager = RedisConnectionManager::from_static();
    let mut conn = static_manager.get_connection().await.unwrap();

    let pong: String = conn.ping().await.unwrap();
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_redis_connect_trait() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    let mut conn = manager.get_connection().await.unwrap();
    let pong: String = conn.ping().await.unwrap();
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_redis_memory_operations() {
    let (container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let memory_test_key = test_key(&container, "memory_test");

    let _: () = conn.set(&memory_test_key, "some_data").await.unwrap();
    let value: String = conn.get(&memory_test_key).await.unwrap();
    assert_eq!(value, "some_data");

    let _: () = conn.del(&memory_test_key).await.unwrap();
    let exists: bool = conn.exists(&memory_test_key).await.unwrap();
    assert!(!exists);
}

// Tests for the new command abstraction
#[tokio::test]
async fn test_redis_commands_abstraction_basic() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let conn = manager.get_connection().await.unwrap();
    let mut commands = conn.cmd();

    // Test ping
    let pong = commands.ping().await.unwrap();
    assert_eq!(pong, "PONG");

    // Test basic string operations
    commands.set("test_key", "test_value").await.unwrap();
    let value: String = commands.get("test_key").await.unwrap();
    assert_eq!(value, "test_value");

    // Test exists and delete
    assert!(commands.exists("test_key").await.unwrap());
    let deleted_count = commands.del("test_key").await.unwrap();
    assert_eq!(deleted_count, 1);
    assert!(!commands.exists("test_key").await.unwrap());
}

#[tokio::test]
async fn test_redis_commands_abstraction_string_ops() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let conn = manager.get_connection().await.unwrap();
    let mut commands = conn.cmd();

    // Test SET with expiration
    commands
        .set_ex("expire_key", "expire_value", 60)
        .await
        .unwrap();
    let ttl = commands.ttl("expire_key").await.unwrap();
    assert!(ttl > 0 && ttl <= 60);

    // Test SET NX (only if not exists)
    let set_result = commands.set_nx("new_key", "new_value").await.unwrap();
    assert!(set_result); // Should return true for new key

    let set_result_again =
        commands.set_nx("new_key", "another_value").await.unwrap();
    assert!(!set_result_again); // Should return false as key exists

    // Test GETSET
    let old_value: String =
        commands.get_set("new_key", "updated_value").await.unwrap();
    assert_eq!(old_value, "new_value");

    let current_value: String = commands.get("new_key").await.unwrap();
    assert_eq!(current_value, "updated_value");
}

#[tokio::test]
async fn test_redis_commands_abstraction_hash_ops() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let conn = manager.get_connection().await.unwrap();
    let mut commands = conn.cmd();

    // Test hash operations
    let set_result =
        commands.hset("hash_key", "field1", "value1").await.unwrap();
    assert!(set_result);

    commands.hset("hash_key", "field2", "value2").await.unwrap();

    let value1: String = commands.hget("hash_key", "field1").await.unwrap();
    assert_eq!(value1, "value1");

    assert!(commands.hexists("hash_key", "field1").await.unwrap());
    assert!(
        !commands
            .hexists("hash_key", "nonexistent_field")
            .await
            .unwrap()
    );

    let all_values: std::collections::HashMap<String, String> =
        commands.hgetall("hash_key").await.unwrap();
    assert_eq!(all_values.len(), 2);
    assert_eq!(all_values.get("field1"), Some(&"value1".to_string()));
    assert_eq!(all_values.get("field2"), Some(&"value2".to_string()));

    let keys: Vec<String> = commands.hkeys("hash_key").await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"field1".to_string()));
    assert!(keys.contains(&"field2".to_string()));

    let values: Vec<String> = commands.hvals("hash_key").await.unwrap();
    assert_eq!(values.len(), 2);
    assert!(values.contains(&"value1".to_string()));
    assert!(values.contains(&"value2".to_string()));

    let deleted_count = commands.hdel("hash_key", "field1").await.unwrap();
    assert_eq!(deleted_count, 1);
    assert!(!commands.hexists("hash_key", "field1").await.unwrap());
}

#[tokio::test]
async fn test_redis_commands_abstraction_list_ops() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let conn = manager.get_connection().await.unwrap();
    let mut commands = conn.cmd();

    // Test list operations
    let length = commands.lpush("list_key", "item1").await.unwrap();
    assert_eq!(length, 1);

    commands.lpush("list_key", "item2").await.unwrap();
    commands.rpush("list_key", "item3").await.unwrap();

    let current_length = commands.llen("list_key").await.unwrap();
    assert_eq!(current_length, 3);

    let items: Vec<String> =
        commands.lrange("list_key", 0, -1).await.unwrap();
    assert_eq!(items, vec!["item2", "item1", "item3"]);

    let popped: Option<String> = commands.lpop("list_key").await.unwrap();
    assert_eq!(popped, Some("item2".to_string()));

    let popped_right: Option<String> =
        commands.rpop("list_key").await.unwrap();
    assert_eq!(popped_right, Some("item3".to_string()));

    let final_length = commands.llen("list_key").await.unwrap();
    assert_eq!(final_length, 1);
}

#[tokio::test]
async fn test_redis_commands_abstraction_set_ops() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let conn = manager.get_connection().await.unwrap();
    let mut commands = conn.cmd();

    // Test set operations
    let added_count = commands.sadd("set_key", "member1").await.unwrap();
    assert_eq!(added_count, 1);

    commands.sadd("set_key", "member2").await.unwrap();
    commands.sadd("set_key", "member3").await.unwrap();

    assert!(commands.sismember("set_key", "member1").await.unwrap());
    assert!(!commands.sismember("set_key", "nonexistent").await.unwrap());

    let cardinality = commands.scard("set_key").await.unwrap();
    assert_eq!(cardinality, 3);

    let members: std::collections::HashSet<String> =
        commands.smembers("set_key").await.unwrap();
    assert_eq!(members.len(), 3);
    assert!(members.contains("member1"));
    assert!(members.contains("member2"));
    assert!(members.contains("member3"));

    let removed_count = commands.srem("set_key", "member1").await.unwrap();
    assert_eq!(removed_count, 1);

    let final_cardinality = commands.scard("set_key").await.unwrap();
    assert_eq!(final_cardinality, 2);
}

#[tokio::test]
async fn test_redis_commands_builder_pattern() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let conn = manager.get_connection().await.unwrap();
    let mut commands = conn.cmd();

    // Test builder pattern for SET with expiration
    let result = commands
        .set_builder("builder_key", "builder_value")
        .expire_in(Duration::from_secs(30))
        .execute(&mut commands)
        .await
        .unwrap();
    assert!(result);

    let ttl = commands.ttl("builder_key").await.unwrap();
    assert!(ttl > 0 && ttl <= 30);

    // Test builder pattern for SET NX
    let nx_result = commands
        .set_builder("nx_key", "nx_value")
        .only_if_not_exists()
        .execute(&mut commands)
        .await
        .unwrap();
    assert!(nx_result);

    let nx_again_result = commands
        .set_builder("nx_key", "another_value")
        .only_if_not_exists()
        .execute(&mut commands)
        .await
        .unwrap();
    assert!(!nx_again_result); // Should fail as key exists
}

#[tokio::test]
async fn test_redis_commands_key_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let conn = manager.get_connection().await.unwrap();
    let mut commands = conn.cmd();

    // Test key operations
    commands.set("old_key", "value").await.unwrap();
    commands.rename("old_key", "new_key").await.unwrap();

    assert!(!commands.exists("old_key").await.unwrap());
    assert!(commands.exists("new_key").await.unwrap());

    let value: String = commands.get("new_key").await.unwrap();
    assert_eq!(value, "value");

    // Test expiration
    assert!(commands.expire("new_key", 60).await.unwrap());
    let ttl = commands.ttl("new_key").await.unwrap();
    assert!(ttl > 0 && ttl <= 60);
}
