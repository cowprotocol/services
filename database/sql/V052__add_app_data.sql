ALTER TABLE orders DROP COLUMN full_app_data;

CREATE TABLE app_data (
    contract_app_data bytea PRIMARY KEY,
    full_app_data bytea NOT NULL
);
