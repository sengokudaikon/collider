-- Create materialized view without data first to ensure structure exists
CREATE MATERIALIZED VIEW IF NOT EXISTS event_hourly_summaries AS
SELECT
    et.name as event_type,
    date_trunc('hour', e.timestamp) as hour,
    COUNT(*) as total_events,
    COUNT(DISTINCT e.user_id) as unique_users
FROM events e
JOIN event_types et ON e.event_type_id = et.id
WHERE e.timestamp >= NOW() - INTERVAL '30 days'
GROUP BY et.name, date_trunc('hour', e.timestamp)
ORDER BY hour DESC, total_events DESC
WITH NO DATA;

-- Daily user activity for engagement tracking
CREATE MATERIALIZED VIEW IF NOT EXISTS user_daily_activity AS
SELECT
    e.user_id,
    date_trunc('day', e.timestamp) as date,
    COUNT(*) as total_events,
    COUNT(DISTINCT et.name) as unique_event_types,
    MIN(e.timestamp) as first_event,
    MAX(e.timestamp) as last_event
FROM events e
JOIN event_types et ON e.event_type_id = et.id
WHERE e.timestamp >= NOW() - INTERVAL '90 days'
GROUP BY e.user_id, date_trunc('day', e.timestamp)
ORDER BY date DESC, total_events DESC
WITH NO DATA;

-- Popular events with growth rate comparison
CREATE MATERIALIZED VIEW IF NOT EXISTS popular_events AS
WITH current_period AS (
    SELECT 
        et.name as event_type,
        'last_7_days' as period,
        COUNT(*) as total_count,
        COUNT(DISTINCT e.user_id) as unique_users
    FROM events e
    JOIN event_types et ON e.event_type_id = et.id
    WHERE e.timestamp >= NOW() - INTERVAL '7 days'
    GROUP BY et.name
),
previous_period AS (
    SELECT 
        et.name as event_type,
        COUNT(*) as prev_count
    FROM events e
    JOIN event_types et ON e.event_type_id = et.id
    WHERE e.timestamp >= NOW() - INTERVAL '14 days'
      AND e.timestamp < NOW() - INTERVAL '7 days'
    GROUP BY et.name
)
SELECT 
    c.event_type,
    c.period,
    c.total_count,
    c.unique_users,
    CASE
        WHEN p.prev_count > 0 THEN
            ROUND(((c.total_count - p.prev_count)::numeric / p.prev_count::numeric) * 100, 2)
        ELSE NULL
    END as growth_rate
FROM current_period c
LEFT JOIN previous_period p ON c.event_type = p.event_type
ORDER BY c.total_count DESC
WITH NO DATA;

-- User session approximations (based on activity gaps)
-- This creates pseudo-sessions by grouping events within 30 minutes of each other
CREATE MATERIALIZED VIEW IF NOT EXISTS user_session_summaries AS
WITH user_session_events AS (
    SELECT 
        e.user_id,
        e.timestamp,
        LAG(e.timestamp) OVER (PARTITION BY e.user_id ORDER BY e.timestamp) as prev_timestamp,
        CASE 
            WHEN LAG(e.timestamp) OVER (PARTITION BY e.user_id ORDER BY e.timestamp) IS NULL 
                 OR e.timestamp - LAG(e.timestamp) OVER (PARTITION BY e.user_id ORDER BY e.timestamp) > INTERVAL '30 minutes'
            THEN 1 
            ELSE 0 
        END as new_session
    FROM events e
    WHERE e.timestamp >= NOW() - INTERVAL '90 days'
),
session_boundaries AS (
    SELECT 
        user_id,
        timestamp,
        SUM(new_session) OVER (PARTITION BY user_id ORDER BY timestamp) as session_id
    FROM user_session_events
),
session_stats AS (
    SELECT 
        user_id,
        session_id,
        MIN(timestamp) as session_start,
        MAX(timestamp) as session_end,
        COUNT(*) as events_in_session,
        EXTRACT(EPOCH FROM (MAX(timestamp) - MIN(timestamp))) as duration_seconds
    FROM session_boundaries  
    GROUP BY user_id, session_id
)
SELECT 
    user_id,
    COUNT(*) as total_sessions,
    AVG(duration_seconds) as avg_session_duration,
    SUM(duration_seconds) as total_time_spent,
    AVG(events_in_session) as avg_events_per_session,
    MIN(session_start) as first_session,
    MAX(session_end) as last_session
FROM session_stats
GROUP BY user_id
ORDER BY total_sessions DESC
WITH NO DATA;