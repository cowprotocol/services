-- Table to store all orders placed via any on-chain broadcaster contract
-- uid is needed as unique identifier for the order
-- Sender is the address who called the broadcasting contract. For ethflow orders, this would be the user placing the order
-- is_reorged is a flag indicated whether the order was placed and then reorged.
-- block_number + log_index are used to deal with chain reverts.
-- All other information about the order is directly translated into an order and the information is stored in the orders table

CREATE TABLE onchain_placed_orders (
    uid bytea PRIMARY KEY,
    sender bytea NOT NULL,
    is_reorged bool NOT NULL,
    block_number bigint NOT NULL,
    log_index bigint NOT NULL
);

-- To get a specific user's order quickly
CREATE INDEX order_sender ON onchain_placed_orders USING HASH (sender);

-- To get most recent events for dealing with reorgs quickly
CREATE INDEX event_index ON onchain_placed_orders USING BTREE (block_number, log_index);