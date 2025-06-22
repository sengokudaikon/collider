-- Drop the refresh function
DROP FUNCTION IF EXISTS refresh_stats_summary();

-- Drop the materialized view (indexes will be dropped automatically)
DROP MATERIALIZED VIEW IF EXISTS stats_summary;