-- MinFee Measurements from the API
CREATE TABLE min_fee_measurements (
  token bytea NOT NULL,
  expiration_timestamp timestamptz NOT NULL,
  min_fee numeric(78,0) NOT NULL
);

-- Get all min fee measurement for a specific token that is not yet expired
CREATE INDEX min_fee_measurements_token_expiration on min_fee_measurements USING BTREE (token, expiration_timestamp DESC);
