-- Create a new table based on the structure of the existing table
CREATE TABLE jit_orders AS
TABLE orders WITH NO DATA;

-- Add the new columns to the newly created table, that represent the event index
ALTER TABLE jit_orders
ADD COLUMN block_number bigint NOT NULL,
ADD COLUMN log_index bigint NOT NULL,
ADD CONSTRAINT jit_orders_pkey PRIMARY KEY (uid);

-- Get a specific user's orders.
CREATE INDEX jit_order_owner ON jit_orders USING HASH (owner);

CREATE INDEX jit_order_creation_timestamp ON jit_orders USING BTREE (creation_timestamp);

-- To get all valid orders quickly, we create an index
CREATE INDEX jit_user_valid_to ON jit_orders USING BTREE (valid_to);

-- To optimize the performance of the user_orders query, we introduce a new index that allows
-- us to quickly get the latest orders from a owner
CREATE INDEX jit_user_order_creation_timestamp ON jit_orders USING BTREE (owner, creation_timestamp DESC);
