-- Migration: Drop analytics materialized views
-- This is the down migration for 004_create_analytics_views.sql

DROP MATERIALIZED VIEW IF EXISTS popular_events CASCADE;
DROP MATERIALIZED VIEW IF EXISTS user_daily_activity CASCADE;
DROP MATERIALIZED VIEW IF EXISTS event_hourly_summaries CASCADE;