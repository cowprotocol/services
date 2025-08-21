-- This adds `creation_timestamp` column to `app_data`, `order_quotes`
-- in order to help data ingestion for our analytics team

ALTER TABLE app_data
    ADD COLUMN creation_timestamp timestamptz NOT NULL DEFAULT '1970-01-01 00:00:00+00'::timestamptz;

ALTER TABLE order_quotes
    ADD COLUMN creation_timestamp timestamptz NOT NULL DEFAULT '1970-01-01 00:00:00+00'::timestamptz;

-- Set default for future inserts
ALTER TABLE app_data
    ALTER COLUMN creation_timestamp SET DEFAULT NOW();

ALTER TABLE order_quotes
    ALTER COLUMN creation_timestamp SET DEFAULT NOW();
