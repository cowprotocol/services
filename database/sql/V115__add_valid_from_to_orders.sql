-- valid_from allows orders to be placed now and only become active in the future
ALTER TABLE orders ADD COLUMN valid_from BIGINT;
