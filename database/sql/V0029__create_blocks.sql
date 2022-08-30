CREATE TABLE blocks (
    block_number bigint NOT NULL,
	block_hash bytea NOT NULL,
    PRIMARY KEY (block_hash)
);
