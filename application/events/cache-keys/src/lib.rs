use events_responses::EventResponse;
use redis_connection::cache_key;

cache_key!(EventCacheKey::<EventResponse> => "event:{}"[id: i64]);
cache_key!(EventListCacheKey::<Vec<EventResponse>> => "events:list:{}"[filter_hash: String]);
cache_key!(UserEventsCacheKey::<Vec<EventResponse>> => "events:user:{}"[user_id: i64]);
cache_key!(UserEventsLimitCacheKey::<Vec<EventResponse>> => "events:user:{}:limit:{}"[user_id: i64, limit: u64]);

cache_key!(EventTypeCacheKey::<String> => "event_type:{}"[id: i32]);
cache_key!(EventTypeListCacheKey::<Vec<String>> => "event_types:list");
