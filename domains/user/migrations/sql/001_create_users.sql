CREATE TABLE IF NOT EXISTS users
(
    id         UUID PRIMARY KEY             DEFAULT uuidv7(),
    name       VARCHAR(100)        NOT NULL,
    created_at TIMESTAMPTZ         NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_users_name ON users (name);
CREATE INDEX IF NOT EXISTS idx_users_created_at ON users (created_at);