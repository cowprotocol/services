-- PreSignature events from the smart contract.
CREATE TABLE presignatures (
    block_number bigint NOT NULL,
    log_index bigint NOT NULL,
    owner bytea NOT NULL,
    order_uid bytea NOT NULL,
    signed boolean NOT NULL,
    PRIMARY KEY (block_number, log_index)
);

CREATE INDEX most_recent_with_orderuid ON presignatures USING BTREE (order_uid, block_number DESC, log_index DESC);
-- Get a specific user's presignature.
CREATE INDEX presignature_owner ON presignatures USING HASH (owner);
