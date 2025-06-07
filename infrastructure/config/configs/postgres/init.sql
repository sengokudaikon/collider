-- High-Performance Event Storage Schema
-- Optimized for fast inserts and efficient querying

-- Enable required extensions
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- Create events table with optimized structure
CREATE TABLE IF NOT EXISTS events (
    id VARCHAR(36) PRIMARY KEY,           -- UUID as string for speed
    data JSONB NOT NULL,                  -- JSONB for efficient JSON operations
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW() NOT NULL
);

-- Create optimized indexes for high-performance queries
CREATE INDEX IF NOT EXISTS idx_events_created_at 
    ON events USING BTREE (created_at DESC);

-- JSONB GIN index for fast JSON queries
CREATE INDEX IF NOT EXISTS idx_events_data_gin 
    ON events USING GIN (data);

-- Partial index for recent events (last 24 hours)
CREATE INDEX IF NOT EXISTS idx_events_recent 
    ON events (created_at) 
    WHERE created_at >= NOW() - INTERVAL '24 hours';

-- Optimize table for high write throughput
ALTER TABLE events SET (
    fillfactor = 90,                      -- Leave space for HOT updates
    autovacuum_vacuum_scale_factor = 0.1, -- More aggressive autovacuum
    autovacuum_analyze_scale_factor = 0.05
);

-- Create a view for recent events (commonly queried)
CREATE OR REPLACE VIEW recent_events AS
SELECT id, data, created_at
FROM events
WHERE created_at >= NOW() - INTERVAL '24 hours'
ORDER BY created_at DESC;

-- Performance monitoring function
CREATE OR REPLACE FUNCTION get_table_stats()
RETURNS TABLE (
    table_name TEXT,
    row_count BIGINT,
    table_size TEXT,
    index_size TEXT,
    total_size TEXT
) AS $$
BEGIN
    RETURN QUERY
    SELECT 
        'events'::TEXT,
        (SELECT COUNT(*) FROM events)::BIGINT,
        pg_size_pretty(pg_total_relation_size('events'::regclass) - pg_indexes_size('events'::regclass)),
        pg_size_pretty(pg_indexes_size('events'::regclass)),
        pg_size_pretty(pg_total_relation_size('events'::regclass));
END;
$$ LANGUAGE plpgsql;

-- Create a function for cleanup old events (for maintenance)
CREATE OR REPLACE FUNCTION cleanup_old_events(days_to_keep INTEGER DEFAULT 30)
RETURNS INTEGER AS $$
DECLARE
    deleted_count INTEGER;
BEGIN
    DELETE FROM events 
    WHERE created_at < NOW() - (days_to_keep || ' days')::INTERVAL;
    
    GET DIAGNOSTICS deleted_count = ROW_COUNT;
    
    -- Force analyze after large delete
    ANALYZE events;
    
    RETURN deleted_count;
END;
$$ LANGUAGE plpgsql;

-- Grant permissions for application user
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'postgres') THEN
        GRANT ALL PRIVILEGES ON ALL TABLES IN SCHEMA public TO postgres;
        GRANT ALL PRIVILEGES ON ALL SEQUENCES IN SCHEMA public TO postgres;
        GRANT EXECUTE ON ALL FUNCTIONS IN SCHEMA public TO postgres;
    END IF;
END $$;