CREATE TYPE OrderClass AS ENUM ('ordinary', 'liquidity', 'limit');
ALTER TABLE orders ADD COLUMN class OrderClass NOT NULL DEFAULT 'ordinary';
UPDATE orders SET class = 'liquidity' WHERE is_liquidity_order;
ALTER TABLE orders ALTER COLUMN class DROP DEFAULT;
