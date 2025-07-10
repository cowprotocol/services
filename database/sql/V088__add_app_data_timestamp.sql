-- Add a timestamp column to app_data table to track when entries are created
-- This is a test migration for flyway testing

ALTER TABLE app_data 
ADD COLUMN created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP NOT NULL; 