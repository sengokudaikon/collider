-- Migration: Drop events table
-- This is the down migration for 003_create_events.sql

DROP TABLE IF EXISTS events CASCADE;