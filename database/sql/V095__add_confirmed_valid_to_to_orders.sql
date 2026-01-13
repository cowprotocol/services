ALTER TABLE orders ADD COLUMN confirmed_valid_to bigint;
UPDATE orders SET confirmed_valid_to = valid_to;
ALTER TABLE orders ALTER COLUMN confirmed_valid_to SET NOT NULL;
