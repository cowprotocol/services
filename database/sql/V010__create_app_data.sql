CREATE TABLE app_data (
    app_data_hash bytea PRIMARY KEY,
    version bytea NOT NULL,
    app_code bytea NOT NULL,
    referrer bytea NOT NULL
);
