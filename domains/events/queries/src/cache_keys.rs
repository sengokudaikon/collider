use events_models::events::EventResponse;
use redis_connection::{json::Json, redis_key};
use uuid::Uuid;

// Tiered cache keys for events domain

// Event entity caching - shorter TTL since events are updated frequently
redis_key!(tier EventCacheKey::<Json<EventResponse>> => "event:{}"[id: Uuid]);
redis_key!(tier EventListCacheKey::<Json<Vec<EventResponse>>> => "events:list:{}"[filter_hash: String]);
redis_key!(tier UserEventsCacheKey::<Json<Vec<EventResponse>>> => "events:user:{}"[user_id: Uuid]);
redis_key!(tier UserEventsLimitCacheKey::<Json<Vec<EventResponse>>> => "events:user:{}:limit:{}"[user_id: Uuid, limit: u64]);

// Event type caching - longer TTL since types rarely change
redis_key!(tier EventTypeCacheKey::<Json<String>> => "event_type:{}"[id: Uuid]);
redis_key!(tier EventTypeListCacheKey::<Json<Vec<String>>> => "event_types:list");
