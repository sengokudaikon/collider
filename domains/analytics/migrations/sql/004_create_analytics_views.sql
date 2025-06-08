CREATE MATERIALIZED VIEW IF NOT EXISTS event_hourly_summaries AS
SELECT et.name                                                          as event_type,
       date_trunc('hour', e.timestamp)                                  as hour,
       COUNT(*)                                                         as total_events,
       COUNT(DISTINCT e.user_id)                                        as unique_users,
       ROUND(COUNT(*)::numeric / COUNT(DISTINCT e.user_id)::numeric, 2) as avg_events_per_user
FROM events e
         JOIN event_types et ON e.event_type_id = et.id
WHERE e.timestamp >= NOW() - INTERVAL '30 days'
GROUP BY et.name, date_trunc('hour', e.timestamp)
ORDER BY hour DESC, total_events DESC;

CREATE MATERIALIZED VIEW IF NOT EXISTS user_daily_activity AS
SELECT e.user_id,
       date_trunc('day', e.timestamp) as date,
       COUNT(*)                       as total_events,
       array_agg(DISTINCT et.name)    as event_types,
       MIN(e.timestamp)               as first_event,
       MAX(e.timestamp)               as last_event
FROM events e
         JOIN event_types et ON e.event_type_id = et.id
WHERE e.timestamp >= NOW() - INTERVAL '90 days'
GROUP BY e.user_id, date_trunc('day', e.timestamp)
ORDER BY date DESC, total_events DESC;

CREATE MATERIALIZED VIEW IF NOT EXISTS popular_events AS
WITH current_period AS (SELECT et.name                   as event_type,
                               'last_7_days'             as period,
                               COUNT(*)                  as total_count,
                               COUNT(DISTINCT e.user_id) as unique_users
                        FROM events e
                                 JOIN event_types et ON e.event_type_id = et.id
                        WHERE e.timestamp >= NOW() - INTERVAL '7 days'
                        GROUP BY et.name),
     previous_period AS (SELECT et.name  as event_type,
                                COUNT(*) as prev_count
                         FROM events e
                                  JOIN event_types et ON e.event_type_id = et.id
                         WHERE e.timestamp >= NOW() - INTERVAL '14 days'
                           AND e.timestamp < NOW() - INTERVAL '7 days'
                         GROUP BY et.name)
SELECT c.event_type,
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
ORDER BY c.total_count DESC;

CREATE INDEX IF NOT EXISTS idx_event_hourly_summaries_hour ON event_hourly_summaries (hour);
CREATE INDEX IF NOT EXISTS idx_event_hourly_summaries_type ON event_hourly_summaries (event_type);
CREATE INDEX IF NOT EXISTS idx_event_hourly_summaries_hour_type ON event_hourly_summaries (hour, event_type);

CREATE INDEX IF NOT EXISTS idx_user_daily_activity_date ON user_daily_activity (date);
CREATE INDEX IF NOT EXISTS idx_user_daily_activity_user ON user_daily_activity (user_id);
CREATE INDEX IF NOT EXISTS idx_user_daily_activity_user_date ON user_daily_activity (user_id, date);

CREATE INDEX IF NOT EXISTS idx_popular_events_period ON popular_events (period);
CREATE INDEX IF NOT EXISTS idx_popular_events_total_count ON popular_events (total_count DESC);

