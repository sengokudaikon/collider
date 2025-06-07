-- Migration: Create events table
-- Created: 2025-06-08

CREATE TABLE IF NOT EXISTS events
(
    id            UUID PRIMARY KEY     DEFAULT gen_random_uuid(),
    user_id       UUID        NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    event_type_id INTEGER     NOT NULL REFERENCES event_types (id) ON DELETE CASCADE,
    metadata      JSONB,
    timestamp     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_events_user_timestamp ON events (user_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_event_type_timestamp ON events (event_type_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_user_type_timestamp ON events (user_id, event_type_id, timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_timestamp_btree ON events (timestamp DESC);
CREATE INDEX IF NOT EXISTS idx_events_metadata_gin ON events USING GIN (metadata);

ALTER TABLE events
    SET (fillfactor = 90);
ALTER TABLE events
    ADD CONSTRAINT check_timestamp_reasonable
        CHECK (timestamp <= NOW() + INTERVAL '1 year' AND timestamp >= '2025-06-08'::timestamp);
ALTER TABLE events
    ADD CONSTRAINT check_metadata_size
        CHECK (octet_length(metadata::text) <= 65536); -- 64KB limit