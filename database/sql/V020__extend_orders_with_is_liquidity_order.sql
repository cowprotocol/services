ALTER TABLE orders ADD is_liquidity_order boolean NOT NULL DEFAULT false;
-- We know that every partially fillable order was also a liquidity order so we
-- initialize the new column accordingly.
-- There have been liquidity orders which were not partially fillable but there is
-- no way to initialize the `is_liquidity_order` fields correctly for those.
-- This is just our best effort to patch up existing orders.
UPDATE orders SET is_liquidity_order = partially_fillable;
-- We only wanted to have a default to make the migration easier. Ultimately
-- we want every new insertion to supply the flag.
ALTER TABLE orders ALTER COLUMN is_liquidity_order DROP DEFAULT;

