use events_models::Event;
use redis_connection::{core::value::Json, redis_key};
use uuid::Uuid;

// Tiered cache keys for events domain

// Event entity caching - shorter TTL since events are updated frequently
redis_key!(EventCacheKey::<Json<Event>> => "event:{}"[id: Uuid]);
redis_key!(EventListCacheKey::<Json<Vec<Event>>> => "events:list:{}"[filter_hash: String]);
redis_key!(UserEventsCacheKey::<Json<Vec<Event>>> => "events:user:{}"[user_id: Uuid]);
redis_key!(UserEventsLimitCacheKey::<Json<Vec<Event>>> => "events:user:{}:limit:{}"[user_id: Uuid, limit: u64]);

// Event type caching - longer TTL since types rarely change
redis_key!(EventTypeCacheKey::<Json<String>> => "event_type:{}"[id: Uuid]);
redis_key!(EventTypeListCacheKey::<Json<Vec<String>>> => "event_types:list");
