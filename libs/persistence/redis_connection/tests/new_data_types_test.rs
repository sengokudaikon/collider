// use deadpool_redis::redis::AsyncCommands;
use std::collections::HashSet;

use redis_connection::{
    cache_key, connection::RedisConnectionManager,
    core::type_bind::CacheTypeBind,
};
use test_utils::redis::TestRedisContainer;

// Test the new Redis data type macros
cache_key!(set UniqueUsers::<String> => "users:active:{}"[date: String]);
cache_key!(zset Leaderboard::<String> => "leaderboard:{}"[game_id: String]);
cache_key!(list RecentActivity::<String> => "activity:{}"[user_id: String]);
cache_key!(stream EventLog::<String> => "events:{}"[stream_id: String]);

async fn setup_test_redis()
-> anyhow::Result<(TestRedisContainer, RedisConnectionManager)> {
    let container = TestRedisContainer::new().await?;
    container.flush_db().await?;
    let manager = RedisConnectionManager::new(container.pool.clone());
    Ok((container, manager))
}

#[tokio::test]
async fn test_redis_set_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    // Test Redis Set using macro
    let unique_users = UniqueUsers;
    let mut set =
        unique_users.bind_with(&mut conn, &"2024-12-14".to_string());

    // Add members to set
    let added: i32 = set.add("user123".to_string()).await.unwrap();
    assert_eq!(added, 1);

    let added2: i32 = set.add("user456".to_string()).await.unwrap();
    assert_eq!(added2, 1);

    // Try adding duplicate - should return 0
    let duplicate: i32 = set.add("user123".to_string()).await.unwrap();
    assert_eq!(duplicate, 0);

    // Check membership
    let contains: bool = set.contains("user123".to_string()).await.unwrap();
    assert!(contains);

    let not_contains: bool =
        set.contains("user999".to_string()).await.unwrap();
    assert!(!not_contains);

    // Get all members
    let members: HashSet<String> = set.members().await.unwrap();
    assert_eq!(members.len(), 2);
    assert!(members.contains("user123"));
    assert!(members.contains("user456"));

    // Get cardinality
    let len: i32 = set.len().await.unwrap();
    assert_eq!(len, 2);

    // Remove member
    let removed: i32 = set.remove("user123".to_string()).await.unwrap();
    assert_eq!(removed, 1);

    let final_len: i32 = set.len().await.unwrap();
    assert_eq!(final_len, 1);
}

#[tokio::test]
async fn test_redis_sorted_set_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    // Test Redis Sorted Set using macro
    let leaderboard = Leaderboard;
    let mut zset = leaderboard.bind_with(&mut conn, &"game_123".to_string());

    // Add members with scores
    let added: i32 = zset
        .add_with_score(100.0, "player1".to_string())
        .await
        .unwrap();
    assert_eq!(added, 1);

    let _: i32 = zset
        .add_with_score(85.0, "player2".to_string())
        .await
        .unwrap();
    let _: i32 = zset
        .add_with_score(120.0, "player3".to_string())
        .await
        .unwrap();

    // Get score of member
    let score: Option<f64> = zset.score("player1".to_string()).await.unwrap();
    assert_eq!(score, Some(100.0));

    // Get rank (0-based, lowest score first)
    let rank: Option<usize> = zset.rank("player1".to_string()).await.unwrap();
    assert_eq!(rank, Some(1)); // player2 (85.0) is rank 0, player1 (100.0) is rank 1

    // Get reverse rank (highest score first)
    let rev_rank: Option<usize> =
        zset.reverse_rank("player3".to_string()).await.unwrap();
    assert_eq!(rev_rank, Some(0)); // player3 has highest score

    // Get range by rank
    let range: Vec<String> = zset.range(0, 1).await.unwrap();
    assert_eq!(range.len(), 2);
    assert_eq!(range[0], "player2"); // Lowest score first
    assert_eq!(range[1], "player1");

    // Get top players (highest scores)
    let top: Vec<(String, f64)> = zset.top(2).await.unwrap();
    assert_eq!(top.len(), 2);
    assert_eq!(top[0], ("player3".to_string(), 120.0));
    assert_eq!(top[1], ("player1".to_string(), 100.0));

    // Get count
    let len: i32 = zset.len().await.unwrap();
    assert_eq!(len, 3);

    // Increment score
    let new_score: f64 = zset
        .increment_score("player2".to_string(), 20.0)
        .await
        .unwrap();
    assert_eq!(new_score, 105.0);
}

#[tokio::test]
async fn test_redis_list_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    // Test Redis List using macro
    let activity = RecentActivity;
    let mut list = activity.bind_with(&mut conn, &"user_123".to_string());

    // Push to left (beginning)
    let len: i32 = list.push_left("login".to_string()).await.unwrap();
    assert_eq!(len, 1);

    let len2: i32 = list.push_left("view_page".to_string()).await.unwrap();
    assert_eq!(len2, 2);

    // Push to right (end)
    let len3: i32 = list.push_right("logout".to_string()).await.unwrap();
    assert_eq!(len3, 3);

    // Get all elements
    let all: Vec<String> = list.all().await.unwrap();
    assert_eq!(all, vec!["view_page", "login", "logout"]);

    // Get specific range
    let range: Vec<String> = list.range(0, 1).await.unwrap();
    assert_eq!(range, vec!["view_page", "login"]);

    // Get element at index
    let first: Option<String> = list.get(0).await.unwrap();
    assert_eq!(first, Some("view_page".to_string()));

    // Get length
    let length: i32 = list.len().await.unwrap();
    assert_eq!(length, 3);

    // Pop from left
    let popped: Option<String> = list.pop_left().await.unwrap();
    assert_eq!(popped, Some("view_page".to_string()));

    // Pop from right
    let popped_right: Option<String> = list.pop_right().await.unwrap();
    assert_eq!(popped_right, Some("logout".to_string()));

    let final_length: i32 = list.len().await.unwrap();
    assert_eq!(final_length, 1);
}

#[tokio::test]
async fn test_redis_stream_operations() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    // Test Redis Stream using macro
    let event_log = EventLog;
    let mut stream =
        event_log.bind_with(&mut conn, &"app_events".to_string());

    // Add entries to stream
    let id1: String = stream
        .add_auto(&[
            ("event", "user_login".to_string()),
            ("user_id", "123".to_string()),
        ])
        .await
        .unwrap();
    assert!(!id1.is_empty());

    let id2: String = stream
        .add_auto(&[
            ("event", "page_view".to_string()),
            ("page", "/dashboard".to_string()),
        ])
        .await
        .unwrap();
    assert!(!id2.is_empty());

    // Get stream length
    let length: i32 = stream.len().await.unwrap();
    assert_eq!(length, 2);

    // Read entries by range
    let range = stream.range("-", "+").await.unwrap();
    assert_eq!(range.ids.len(), 2);

    // Read with count limit
    let limited_range = stream.range_count("-", "+", 1).await.unwrap();
    assert_eq!(limited_range.ids.len(), 1);

    // Test stream info
    let info = stream.info().await.unwrap();
    assert_eq!(info.length, 2);
    assert!(!info.first_entry.id.is_empty());
    assert!(!info.last_entry.id.is_empty());
}

#[tokio::test]
async fn test_redis_stream_consumer_groups() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    let event_log = EventLog;
    let mut stream =
        event_log.bind_with(&mut conn, &"group_events".to_string());

    // Add some entries first
    stream
        .add_auto(&[("event", "test1".to_string())])
        .await
        .unwrap();
    stream
        .add_auto(&[("event", "test2".to_string())])
        .await
        .unwrap();

    // Create consumer group
    let created: String =
        stream.create_group("processors", "0").await.unwrap();
    assert_eq!(created, "OK");

    // Get groups info
    let groups_info = stream.info_groups().await.unwrap();
    assert_eq!(groups_info.groups.len(), 1);
    assert_eq!(groups_info.groups[0].name, "processors");

    // Read from group (should get messages)
    let messages = stream
        .read_group("processors", "consumer1", ">")
        .await
        .unwrap();
    assert_eq!(messages.keys.len(), 1);

    if let Some(key_data) = messages.keys.first() {
        assert!(!key_data.ids.is_empty());
    }
}

#[tokio::test]
async fn test_backward_compatibility() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    // Test that existing hash and normal operations still work
    cache_key!(hash UserProfiles::<String> => "user:profile:{}"[user_id: String]);
    cache_key!(UserSession::<String> => "session:{}"[session_id: String]);

    let profiles = UserProfiles;
    let mut hash = profiles.bind_with(&mut conn, &"user_123".to_string());

    let set_result: i32 =
        hash.set("name", "John Doe".to_string()).await.unwrap();
    assert_eq!(set_result, 1);

    let get_result: String = hash.get("name").await.unwrap();
    assert_eq!(get_result, "John Doe");

    let session_key = UserSession;
    let mut session =
        session_key.bind_with(&mut conn, &"sess_456".to_string());

    let _: String = session
        .set("active_session_data".to_string())
        .await
        .unwrap();
    let retrieved: String = session.get().await.unwrap();
    assert_eq!(retrieved, "active_session_data");
}

#[tokio::test]
async fn test_multiple_data_types_together() {
    let (_container, manager) = setup_test_redis().await.unwrap();
    let mut conn = manager.get_connection().await.unwrap();

    // Test using multiple data types for a complete scenario
    let date = "2024-12-14".to_string();
    let user_id = "user_123".to_string();
    let game_id = "game_456".to_string();

    // Track active users in a set
    {
        let unique_users = UniqueUsers;
        let mut user_set = unique_users.bind_with(&mut conn, &date);
        let _: i32 = user_set.add(user_id.clone()).await.unwrap();
        let user_count: i32 = user_set.len().await.unwrap();
        assert_eq!(user_count, 1);
    }

    // Track user activity in a list
    {
        let activity = RecentActivity;
        let mut activity_list = activity.bind_with(&mut conn, &user_id);
        let _: i32 = activity_list
            .push_right("started_game".to_string())
            .await
            .unwrap();
        let _: i32 = activity_list
            .push_right("scored_points".to_string())
            .await
            .unwrap();
        let activities: Vec<String> = activity_list.all().await.unwrap();
        assert_eq!(activities.len(), 2);
    }

    // Update leaderboard with sorted set
    {
        let leaderboard = Leaderboard;
        let mut scores = leaderboard.bind_with(&mut conn, &game_id);
        let _: i32 =
            scores.add_with_score(150.0, user_id.clone()).await.unwrap();
        let user_score: Option<f64> =
            scores.score(user_id.clone()).await.unwrap();
        assert_eq!(user_score, Some(150.0));
    }

    // Log events in stream
    {
        let event_log = EventLog;
        let mut events =
            event_log.bind_with(&mut conn, &"game_events".to_string());
        events
            .add_auto(&[
                ("user_id", user_id.clone()),
                ("action", "game_start".to_string()),
            ])
            .await
            .unwrap();
        let event_count: i32 = events.len().await.unwrap();
        assert_eq!(event_count, 1);
    }
}
