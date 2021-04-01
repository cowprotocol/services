CREATE TABLE settlements
(
    block_number bigint NOT NULL,
    log_index bigint NOT NULL,
    solver    bytea NOT NULL,
    tx_hash   bytea  NOT NULL,

    PRIMARY KEY (block_number, log_index)
);
