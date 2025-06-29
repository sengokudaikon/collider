use events_models::Event;
use redis_connection::cache_key;

cache_key!(EventCacheKey::<Event> => "event:{}"[id: i64]);
cache_key!(EventListCacheKey::<Vec<Event>> => "events:list:{}"[filter_hash: String]);
cache_key!(UserEventsCacheKey::<Vec<Event>> => "events:user:{}"[user_id: i64]);
cache_key!(UserEventsLimitCacheKey::<Vec<Event>> => "events:user:{}:limit:{}"[user_id: i64, limit: u64]);

cache_key!(EventTypeCacheKey::<String> => "event_type:{}"[id: i32]);
cache_key!(EventTypeListCacheKey::<Vec<String>> => "event_types:list");
