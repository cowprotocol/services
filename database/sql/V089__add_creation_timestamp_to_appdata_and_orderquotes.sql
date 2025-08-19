
-- Add the columns as nullable first
ALTER TABLE app_data
    ADD COLUMN creation_timestamp timestamptz;

ALTER TABLE order_quotes
    ADD COLUMN creation_timestamp timestamptz;

-- Set default for future inserts
ALTER TABLE app_data
    ALTER COLUMN creation_timestamp SET DEFAULT NOW();

ALTER TABLE order_quotes
    ALTER COLUMN creation_timestamp SET DEFAULT NOW();

-- Update existing rows with sentinel value (Unix epoch)
UPDATE app_data
SET creation_timestamp = '1970-01-01 00:00:00+00'::timestamptz
WHERE creation_timestamp IS NULL;

UPDATE order_quotes
SET creation_timestamp = '1970-01-01 00:00:00+00'::timestamptz
WHERE creation_timestamp IS NULL;

-- Make the columns NOT NULL
ALTER TABLE app_data
    ALTER COLUMN creation_timestamp SET NOT NULL;

ALTER TABLE order_quotes
    ALTER COLUMN creation_timestamp SET NOT NULL;
