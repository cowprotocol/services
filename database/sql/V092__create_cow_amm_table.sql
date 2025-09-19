-- Create table to store information about indexed CoW AMMs
CREATE TABLE cow_amms (
    address BYTEA NOT NULL PRIMARY KEY,
    tradeable_tokens BYTEA[] NOT NULL,
    block_number BIGINT NOT NULL
);
