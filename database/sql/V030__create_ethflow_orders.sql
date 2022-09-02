-- table to store additional information only available for ethflow orders
-- uid is needed as unique identifier for the order
-- the user_valid_to is the only additional information

CREATE TABLE ethflow_orders (
    uid bytea PRIMARY KEY,
    valid_to bigint NOT NULL
);

-- to get all valid orders quickly, we create an index
CREATE INDEX user_valid_to ON orders USING BTREE (valid_to);
