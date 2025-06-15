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

-- GIN indexes for JSONB metadata queries
CREATE INDEX IF NOT EXISTS idx_events_metadata_gin ON events USING GIN (metadata);

-- B-tree indexes for specific metadata field access patterns
CREATE INDEX IF NOT EXISTS idx_events_metadata_page ON events USING BTREE ((metadata->>'page')) 
WHERE metadata ? 'page';

CREATE INDEX IF NOT EXISTS idx_events_metadata_product_id ON events USING BTREE (((metadata->>'product_id')::integer)) 
WHERE metadata ? 'product_id';

CREATE INDEX IF NOT EXISTS idx_events_metadata_session_id ON events USING BTREE ((metadata->>'session_id')) 
WHERE metadata ? 'session_id';

CREATE INDEX IF NOT EXISTS idx_events_metadata_referrer ON events USING BTREE ((metadata->>'referrer')) 
WHERE metadata ? 'referrer';

-- Composite indexes for analytics performance
CREATE INDEX IF NOT EXISTS idx_events_timestamp_page ON events (timestamp, (metadata->>'page'))
WHERE metadata ? 'page';

CREATE INDEX IF NOT EXISTS idx_events_timestamp_product_id ON events (timestamp, ((metadata->>'product_id')::integer))
WHERE metadata ? 'product_id';

-- Indexes for new materialized views
CREATE INDEX IF NOT EXISTS idx_page_analytics_hour ON page_analytics (hour);
CREATE INDEX IF NOT EXISTS idx_page_analytics_page ON page_analytics (page);
CREATE UNIQUE INDEX IF NOT EXISTS idx_page_analytics_unique ON page_analytics (page, hour);

CREATE INDEX IF NOT EXISTS idx_product_analytics_date ON product_analytics (date);
CREATE INDEX IF NOT EXISTS idx_product_analytics_product_id ON product_analytics (product_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_product_analytics_unique ON product_analytics (product_id, event_type, date);

CREATE INDEX IF NOT EXISTS idx_referrer_analytics_date ON referrer_analytics (date);
CREATE INDEX IF NOT EXISTS idx_referrer_analytics_referrer ON referrer_analytics (referrer);
CREATE UNIQUE INDEX IF NOT EXISTS idx_referrer_analytics_unique ON referrer_analytics (referrer, date);

REFRESH MATERIALIZED VIEW event_hourly_summaries;
REFRESH MATERIALIZED VIEW popular_events;
REFRESH MATERIALIZED VIEW user_daily_activity;
REFRESH MATERIALIZED VIEW user_session_summaries;
REFRESH MATERIALIZED VIEW page_analytics;
REFRESH MATERIALIZED VIEW product_analytics;
REFRESH MATERIALIZED VIEW referrer_analytics;