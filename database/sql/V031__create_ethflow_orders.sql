-- Table to store additional information available for ethflow orders
-- uid is needed as unique identifier for the order
-- The user_valid_to is the valid_to specified by the user

CREATE TABLE ethflow_orders (
    uid bytea PRIMARY KEY,
    valid_to bigint NOT NULL
);

-- To get all valid orders quickly, we create an index
CREATE INDEX user_valid_to ON orders USING BTREE (valid_to);
