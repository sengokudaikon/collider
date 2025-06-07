use deadpool_redis::redis::AsyncCommands;
use redis_connection::{
    config::{DbConnectConfig, RedisDbConfig},
    connection::{RedisConnect, RedisConnectionManager},
};
use test_utils::redis::TestRedisContainer;

async fn setup_test_redis()
-> anyhow::Result<(TestRedisContainer, RedisConnectionManager)> {
    let container = TestRedisContainer::new().await?;
    let manager = RedisConnectionManager::new(container.pool.clone());
    Ok((container, manager))
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

    // Test that RedisDbConfig implements DbConnectConfig trait correctly
    assert_eq!(config.host(), "test.redis.host");
    assert_eq!(config.port(), 6380);
    assert_eq!(config.db(), 1);
    assert_eq!(config.password(), None);
}

#[tokio::test]
async fn test_redis_connection_manager() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    // Test basic connection
    let mut conn = manager.get_connection().await.unwrap();

    // Test PING command
    let pong: String = conn.ping().await.unwrap();
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_redis_connection_manager_mut() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    // Test mutable connection
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Test SET/GET commands
    let _: () = conn.set("test_key", "test_value").await.unwrap();
    let value: String = conn.get("test_key").await.unwrap();
    assert_eq!(value, "test_value");
}

#[tokio::test]
async fn test_redis_basic_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Test set/get string
    let _: () = conn.set("string_key", "hello world").await.unwrap();
    let value: String = conn.get("string_key").await.unwrap();
    assert_eq!(value, "hello world");

    // Test set/get integer
    let _: () = conn.set("int_key", 42i32).await.unwrap();
    let int_value: i32 = conn.get("int_key").await.unwrap();
    assert_eq!(int_value, 42);

    // Test exists
    let exists: bool = conn.exists("string_key").await.unwrap();
    assert!(exists);

    let not_exists: bool = conn.exists("nonexistent_key").await.unwrap();
    assert!(!not_exists);

    // Test delete
    let _: () = conn.del("string_key").await.unwrap();
    let exists_after_del: bool = conn.exists("string_key").await.unwrap();
    assert!(!exists_after_del);
}

#[tokio::test]
async fn test_redis_hash_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Test hash set/get
    let _: () = conn.hset("hash_key", "field1", "value1").await.unwrap();
    let _: () = conn.hset("hash_key", "field2", "value2").await.unwrap();

    let value1: String = conn.hget("hash_key", "field1").await.unwrap();
    assert_eq!(value1, "value1");

    let value2: String = conn.hget("hash_key", "field2").await.unwrap();
    assert_eq!(value2, "value2");

    // Test hash exists
    let field_exists: bool =
        conn.hexists("hash_key", "field1").await.unwrap();
    assert!(field_exists);

    let field_not_exists: bool =
        conn.hexists("hash_key", "nonexistent_field").await.unwrap();
    assert!(!field_not_exists);

    // Test hash get all
    let all_fields: std::collections::HashMap<String, String> =
        conn.hgetall("hash_key").await.unwrap();
    assert_eq!(all_fields.len(), 2);
    assert_eq!(all_fields.get("field1"), Some(&"value1".to_string()));
    assert_eq!(all_fields.get("field2"), Some(&"value2".to_string()));

    // Test hash delete field
    let _: () = conn.hdel("hash_key", "field1").await.unwrap();
    let field_exists_after_del: bool =
        conn.hexists("hash_key", "field1").await.unwrap();
    assert!(!field_exists_after_del);
}

#[tokio::test]
async fn test_redis_list_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Test list push/pop
    let _: () = conn.lpush("list_key", "item1").await.unwrap();
    let _: () = conn.lpush("list_key", "item2").await.unwrap();
    let _: () = conn.rpush("list_key", "item3").await.unwrap();

    let length: i32 = conn.llen("list_key").await.unwrap();
    assert_eq!(length, 3);

    let items: Vec<String> = conn.lrange("list_key", 0, -1).await.unwrap();
    assert_eq!(items, vec!["item2", "item1", "item3"]);

    let popped: String = conn.lpop("list_key", None).await.unwrap();
    assert_eq!(popped, "item2");

    let length_after_pop: i32 = conn.llen("list_key").await.unwrap();
    assert_eq!(length_after_pop, 2);
}

#[tokio::test]
async fn test_redis_set_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Test set operations
    let _: () = conn.sadd("set_key", "member1").await.unwrap();
    let _: () = conn.sadd("set_key", "member2").await.unwrap();
    let _: () = conn.sadd("set_key", "member3").await.unwrap();

    let is_member: bool = conn.sismember("set_key", "member1").await.unwrap();
    assert!(is_member);

    let not_member: bool =
        conn.sismember("set_key", "nonexistent").await.unwrap();
    assert!(!not_member);

    let cardinality: i32 = conn.scard("set_key").await.unwrap();
    assert_eq!(cardinality, 3);

    let members: std::collections::HashSet<String> =
        conn.smembers("set_key").await.unwrap();
    assert_eq!(members.len(), 3);
    assert!(members.contains("member1"));
    assert!(members.contains("member2"));
    assert!(members.contains("member3"));

    let _: () = conn.srem("set_key", "member1").await.unwrap();
    let cardinality_after_remove: i32 = conn.scard("set_key").await.unwrap();
    assert_eq!(cardinality_after_remove, 2);
}

#[tokio::test]
async fn test_redis_expire_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Test TTL operations
    let _: () = conn.set("temp_key", "temporary_value").await.unwrap();
    let _: () = conn.expire("temp_key", 60).await.unwrap();

    let ttl: i32 = conn.ttl("temp_key").await.unwrap();
    assert!(ttl > 0 && ttl <= 60);

    // Test persist (remove TTL)
    let _: () = conn.persist("temp_key").await.unwrap();
    let ttl_after_persist: i32 = conn.ttl("temp_key").await.unwrap();
    assert_eq!(ttl_after_persist, -1); // -1 means no TTL
}

#[tokio::test]
async fn test_redis_key_pattern_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Set up some test keys
    let _: () = conn.set("pattern:1", "value1").await.unwrap();
    let _: () = conn.set("pattern:2", "value2").await.unwrap();
    let _: () = conn.set("other:key", "value3").await.unwrap();

    // Test keys pattern matching
    let keys: Vec<String> = conn.keys("pattern:*").await.unwrap();
    assert_eq!(keys.len(), 2);
    assert!(keys.contains(&"pattern:1".to_string()));
    assert!(keys.contains(&"pattern:2".to_string()));

    // Test type detection
    let key_type: String = conn.key_type("pattern:1").await.unwrap();
    assert_eq!(key_type, "string");

    // Test rename
    let _: () = conn.rename("pattern:1", "renamed:1").await.unwrap();
    let exists_old: bool = conn.exists("pattern:1").await.unwrap();
    let exists_new: bool = conn.exists("renamed:1").await.unwrap();
    assert!(!exists_old);
    assert!(exists_new);
}

#[tokio::test]
async fn test_concurrent_redis_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    // Test concurrent operations
    let tasks = (0..10).map(|i| {
        let manager = manager.clone();
        tokio::spawn(async move {
            let mut conn = manager.get_mut_connection().await.unwrap();
            let key = format!("concurrent:key:{}", i);
            let value = format!("value_{}", i);

            let _: () = conn.set(&key, &value).await.unwrap();
            let retrieved: String = conn.get(&key).await.unwrap();
            assert_eq!(retrieved, value);
        })
    });

    // Wait for all tasks to complete
    for task in tasks {
        task.await.unwrap();
    }

    // Verify all keys were set
    let mut conn = manager.get_mut_connection().await.unwrap();
    let keys: Vec<String> = conn.keys("concurrent:key:*").await.unwrap();
    assert_eq!(keys.len(), 10);
}

#[tokio::test]
async fn test_redis_connection_manager_static() {
    let (_container, _manager) = setup_test_redis().await.unwrap();

    // Test static initialization - note: we can't access private pool field
    // so we'll test the static functionality differently
    let another_container = TestRedisContainer::new().await.unwrap();
    RedisConnectionManager::init_static(another_container.pool);

    let static_manager = RedisConnectionManager::from_static();
    let mut conn = static_manager.get_connection().await.unwrap();

    // Test basic operation with static manager
    let pong: String = conn.ping().await.unwrap();
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_redis_connect_trait() {
    let (_container, manager) = setup_test_redis().await.unwrap();

    // Test RedisConnect trait
    let mut conn = manager.get_connection().await.unwrap();
    let pong: String = conn.ping().await.unwrap();
    assert_eq!(pong, "PONG");
}

#[tokio::test]
async fn test_redis_memory_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_mut_connection().await.unwrap();

    // Test memory operations
    let _: () = conn.set("memory_test", "some_data").await.unwrap();
    let value: String = conn.get("memory_test").await.unwrap();
    assert_eq!(value, "some_data");

    // Test memory cleanup
    let _: () = conn.del("memory_test").await.unwrap();
    let exists: bool = conn.exists("memory_test").await.unwrap();
    assert!(!exists);
}
