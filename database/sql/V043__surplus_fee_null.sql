UPDATE orders SET surplus_fee = 0 WHERE class = 'limit' AND surplus_fee IS NULL;
UPDATE orders SET surplus_fee_timestamp = '1970-01-01' WHERE class = 'limit' AND surplus_fee_timestamp IS NULL;
