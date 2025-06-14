-- Migration: Drop event_types table
-- This is the down migration for 001_create_event_types.sql

DROP TABLE IF EXISTS event_types CASCADE;