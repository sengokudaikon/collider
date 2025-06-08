CREATE TABLE IF NOT EXISTS event_types
(
    id   SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL UNIQUE
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_event_types_name ON event_types (name);