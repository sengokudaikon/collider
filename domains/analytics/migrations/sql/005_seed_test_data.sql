-- Seed minimal test data for materialized views to work
-- This ensures that the analytics views can be created successfully

-- Insert a test user if none exists
INSERT INTO users (id, name, created_at)
SELECT 
    '00000000-0000-0000-0000-000000000001'::UUID,
    'System User',
    NOW()
WHERE NOT EXISTS (
    SELECT 1 FROM users WHERE id = '00000000-0000-0000-0000-000000000001'::UUID
);

-- Insert basic event types if they don't exist
INSERT INTO event_types (id, name)
SELECT * FROM (VALUES
    (1, 'page_view'),
    (2, 'user_login' ),
    (3, 'user_logout'),
    (4, 'button_click')
) AS v(id, name)
WHERE NOT EXISTS (
    SELECT 1 FROM event_types WHERE id = v.id
);

-- Insert some sample events to ensure views can be created
INSERT INTO events (id, event_type_id, user_id, timestamp, metadata)
SELECT 
    gen_random_uuid(),
    (ARRAY[1, 2, 3, 4])[floor(random() * 4 + 1)],
    '00000000-0000-0000-0000-000000000001'::UUID,
    NOW() - (random() * INTERVAL '30 days'),
    '{}'::JSONB
FROM generate_series(1, 10)
WHERE NOT EXISTS (
    SELECT 1 FROM events LIMIT 1
);


-- Create indexes on materialized views
-- Note: These indexes are created on the structure of the materialized view, not the data
CREATE INDEX IF NOT EXISTS idx_event_hourly_summaries_hour ON event_hourly_summaries (hour);
CREATE INDEX IF NOT EXISTS idx_event_hourly_summaries_type ON event_hourly_summaries (event_type);
CREATE INDEX IF NOT EXISTS idx_event_hourly_summaries_hour_type ON event_hourly_summaries (hour, event_type);
CREATE UNIQUE INDEX IF NOT EXISTS idx_event_hourly_summaries_unique ON event_hourly_summaries (event_type, hour);

CREATE INDEX IF NOT EXISTS idx_user_daily_activity_date ON user_daily_activity (date);
CREATE INDEX IF NOT EXISTS idx_user_daily_activity_user ON user_daily_activity (user_id);
CREATE INDEX IF NOT EXISTS idx_user_daily_activity_user_date ON user_daily_activity (user_id, date);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_daily_activity_unique ON user_daily_activity (user_id, date);

CREATE INDEX IF NOT EXISTS idx_popular_events_period ON popular_events (period);
CREATE INDEX IF NOT EXISTS idx_popular_events_total_count ON popular_events (total_count DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_popular_events_unique ON popular_events (event_type, period);

CREATE INDEX IF NOT EXISTS idx_user_session_summaries_user ON user_session_summaries (user_id);
CREATE INDEX IF NOT EXISTS idx_user_session_summaries_total_sessions ON user_session_summaries (total_sessions DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_user_session_summaries_unique ON user_session_summaries (user_id);