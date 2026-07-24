CREATE INDEX CONCURRENTLY orders_valid_from_idx ON orders (valid_from)
WHERE valid_from IS NOT NULL;
