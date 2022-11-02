ALTER TABLE orders ADD COLUMN surplus_fee numeric(78,0);
ALTER TABLE orders ADD COLUMN surplus_fee_timestamp timestamptz;
