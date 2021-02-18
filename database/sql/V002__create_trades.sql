-- Trade events from the smart contract.
CREATE TABLE trades (
    block_number bigint NOT NULL,
    log_index bigint NOT NULL,
    -- Not foreign key because there can be trade events for orders we don't know.
    order_uid bytea NOT NULL,
    sell_amount numeric(78,0) NOT NULL,
    buy_amount numeric(78,0) NOT NULL,
    fee_amount numeric(78,0) NOT NULL,
    PRIMARY KEY (block_number, log_index)
);

-- Get all trades belonging to an order.
CREATE INDEX trade_order_uid on trades USING BTREE (order_uid, block_number, log_index);
