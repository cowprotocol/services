-- PreSignature events from the smart contract.
CREATE TABLE presignatures (
    block_number bigint NOT NULL,
    log_index bigint NOT NULL,
    owner bytea NOT NULL,
    order_uid bytea NOT NULL,
    signed boolean NOT NULL,
    PRIMARY KEY (block_number, log_index)
);

CREATE INDEX order_uid ON presignatures USING HASH (order_uid);
-- Get a specific user's presignature.
CREATE INDEX presignature_owner ON presignatures USING HASH (owner);
