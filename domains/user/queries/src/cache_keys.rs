use redis_connection::{json::Json, redis_key};
use user_models::Model as User;
use uuid::Uuid;

use crate::user_analytics::{EventTypeCount, UserEventMetrics};

// Tiered cache keys for user domain

// User entity caching
redis_key!(tier UserCacheKey::<Json<User>> => "user:{}"[id: Uuid]);
redis_key!(tier UserByNameCacheKey::<Json<User>> => "user:name:{}"[name: String]);
redis_key!(tier UserListCacheKey::<Json<Vec<User>>> => "users:list");

// User analytics caching
redis_key!(tier UserMetricsCacheKey::<Json<UserEventMetrics>> => "user:{}:metrics"[id: Uuid]);
redis_key!(tier UserEventCountCacheKey::<Json<u64>> => "user:{}:events:{}:{}"[user_id: Uuid, period: String, bucket: String]);
redis_key!(tier UserEventTypesCacheKey::<Json<Vec<EventTypeCount>>> => "user:{}:event_types:{}:{}"[user_id: Uuid, start: String, end: String]);
redis_key!(tier UserTotalEventsCacheKey::<Json<u64>> => "user:{}:total_events"[id: Uuid]);

// Individual bucket operations caching
redis_key!(tier BucketCountCacheKey::<Json<u64>> => "{}"[bucket_key: String]);
redis_key!(tier UserEventKeysPatternCacheKey::<Json<Vec<String>>> => "pattern:{}"[pattern: String]);

// Batch operations caching
redis_key!(tier BatchUserMetricsCacheKey::<Json<Vec<(Uuid, UserEventMetrics)>>> => "users:batch_metrics:{}"[ids_hash: String]);
