CREATE TABLE app_data (
    app_data_hash bytea PRIMARY KEY,
    app_code bytea,
    referrer bytea,
    file_blob jsonb NOT NULL
);
-- Get a specific referral.
CREATE INDEX referrer ON app_data USING BTREE (referrer);

-- Get a specific app_code.
CREATE INDEX app_code ON app_data USING BTREE (app_code);