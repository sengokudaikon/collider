-- Migration: Drop users table and related objects
-- This is the down migration for 001_create_users.sql

-- Drop the trigger first
DROP TRIGGER IF EXISTS update_users_updated_at ON users;

-- Drop the function
DROP FUNCTION IF EXISTS update_updated_at_column();

-- Drop the table
DROP TABLE IF EXISTS users CASCADE;