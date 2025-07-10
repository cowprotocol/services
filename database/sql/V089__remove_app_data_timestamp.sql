-- Remove the timestamp column from app_data table
-- This is a rollback migration for testing purposes

ALTER TABLE app_data 
DROP COLUMN created_at; 