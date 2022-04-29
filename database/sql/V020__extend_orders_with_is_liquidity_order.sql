ALTER TABLE orders ADD is_liquidity_order boolean NOT NULL DEFAULT false;
-- So far every liquidity order was also a partially fillable order and all
-- partially fillable orders were liquidity orders.
-- We use this fact to initialize is_liquidity_order for existing orders correctly.
UPDATE orders SET is_liquidity_order = partially_fillable WHERE is_liquidity_order = false;
