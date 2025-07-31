
ALTER TABLE app_data
    ADD COLUMN creation_timestamp timestamptz DEFAULT NOW();

ALTER TABLE order_quotes
    ADD COLUMN creation_timestamp timestamptz DEFAULT NOW();
