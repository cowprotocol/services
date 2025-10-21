-- Create table to store information about indexed CoW AMMs
CREATE TABLE cow_amms (
    address BYTEA NOT NULL PRIMARY KEY,
    factory_address BYTEA NOT NULL,
    tradeable_tokens BYTEA[] NOT NULL,
    block_number BIGINT NOT NULL,
    tx_hash BYTEA NOT NULL
);

-- Index for efficient reorg handling (delete by factory and block range)
CREATE INDEX cow_amms_factory_block ON cow_amms (factory_address, block_number);
