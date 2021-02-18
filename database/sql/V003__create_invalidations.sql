-- OrderInvalidated events from the smart contract.
CREATE TABLE invalidations (
    block_number bigint NOT NULL,
    log_index bigint NOT NULL,
    order_uid bytea NOT NULL,
    PRIMARY KEY (block_number, log_index)
);

-- Get all invalidations belonging to an order.
CREATE INDEX invalidations_order_uid on invalidations USING BTREE (order_uid, block_number, log_index);
