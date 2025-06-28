CREATE MATERIALIZED VIEW stats_summary AS
WITH event_stats AS (
    SELECT 
        et.name as event_type,
        COUNT(e.id) as total_count,
        COUNT(DISTINCT e.user_id) as unique_users,
        DATE_TRUNC('hour', e.timestamp) as hour_bucket
    FROM events e
    INNER JOIN event_types et ON e.event_type_id = et.id
    WHERE e.timestamp >= NOW() - INTERVAL '30 days'  -- Keep last 30 days
    GROUP BY et.name, DATE_TRUNC('hour', e.timestamp)
),
page_stats AS (
    SELECT 
        COALESCE(e.metadata->>'page', 'unknown') as page,
        COUNT(e.id) as count,
        DATE_TRUNC('hour', e.timestamp) as hour_bucket
    FROM events e
    WHERE e.metadata ? 'page'
      AND e.timestamp >= NOW() - INTERVAL '30 days'
    GROUP BY e.metadata->>'page', DATE_TRUNC('hour', e.timestamp)
)
SELECT 
    'event_type' as stat_type,
    event_type as key_name,
    hour_bucket,
    total_count,
    unique_users,
    NULL::bigint as page_count
FROM event_stats

UNION ALL

SELECT 
    'page' as stat_type,
    page as key_name,
    hour_bucket,
    NULL::bigint as total_count,
    NULL::bigint as unique_users,
    count as page_count
FROM page_stats;

CREATE INDEX IF NOT EXISTS idx_stats_summary_stat_type ON stats_summary (stat_type);
CREATE INDEX IF NOT EXISTS idx_stats_summary_hour_bucket ON stats_summary (hour_bucket);
CREATE INDEX IF NOT EXISTS idx_stats_summary_key_name ON stats_summary (key_name);
CREATE UNIQUE INDEX idx_stats_summary_unique_composite ON stats_summary (stat_type, hour_bucket, key_name);

GRANT SELECT ON stats_summary TO PUBLIC;

REFRESH MATERIALIZED VIEW  stats_summary;