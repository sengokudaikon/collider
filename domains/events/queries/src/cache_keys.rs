use events_models::Event;
use redis_connection::{cache_key, core::value::Json};
use uuid::Uuid;

// Tiered cache keys for events domain

// Event entity caching - shorter TTL since events are updated frequently
cache_key!(EventCacheKey::<Json<Event>> => "event:{}"[id: Uuid]);
cache_key!(EventListCacheKey::<Json<Vec<Event>>> => "events:list:{}"[filter_hash: String]);
cache_key!(UserEventsCacheKey::<Json<Vec<Event>>> => "events:user:{}"[user_id: Uuid]);
cache_key!(UserEventsLimitCacheKey::<Json<Vec<Event>>> => "events:user:{}:limit:{}"[user_id: Uuid, limit: u64]);

// Event type caching - longer TTL since types rarely change
cache_key!(EventTypeCacheKey::<Json<String>> => "event_type:{}"[id: Uuid]);
cache_key!(EventTypeListCacheKey::<Json<Vec<String>>> => "event_types:list");
