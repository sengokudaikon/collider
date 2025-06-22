CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_user_id ON events (user_id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_timestamp ON events (timestamp DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_user_id_timestamp ON events (user_id, timestamp DESC);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_event_type_id ON events (event_type_id);

CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_events_metadata_gin ON events USING GIN (metadata);

REFRESH MATERIALIZED VIEW CONCURRENTLY stats_summary;