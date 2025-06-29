CREATE TABLE IF NOT EXISTS events
(
    id            BIGSERIAL PRIMARY KEY,
    user_id       BIGINT      NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    event_type_id INTEGER     NOT NULL REFERENCES event_types (id) ON DELETE CASCADE,
    metadata      JSONB,
    timestamp     TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

ALTER TABLE events SET (fillfactor = 90);