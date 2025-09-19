-- Create table to store information about indexed CoW AMMs
CREATE TABLE cow_amms (
    address BYTEA NOT NULL PRIMARY KEY,
    helper_contract_address BYTEA NOT NULL,
    tradeable_tokens BYTEA[] NOT NULL
);

CREATE INDEX idx_cow_amms_helper_contract ON cow_amms (helper_contract_address);
