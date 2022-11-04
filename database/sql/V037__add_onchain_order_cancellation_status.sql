-- Table to store all canellations of onchain orders
-- uid is needed as unique identifier for the order.
-- is_reorged is a flag indicated whether the cancellation was mined and then reorged.
-- block_number + log_index are used to deal with chain reverts.

CREATE TABLE onchain_order_cancellations (
    uid bytea PRIMARY KEY,
    is_reorged bool NOT NULL,
    block_number bigint NOT NULL,
    log_index bigint NOT NULL
);

-- To get most recent events for dealing with reorgs quickly
CREATE INDEX cancellation_event_index ON onchain_order_cancellations USING BTREE (block_number, log_index);
