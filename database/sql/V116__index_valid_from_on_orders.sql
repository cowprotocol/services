-- index to quickly find orders that are already active. most orders are
-- expected to be active immediately so we use a partial index to keep it
-- small
CREATE INDEX CONCURRENTLY orders_valid_from_idx ON orders (valid_from)
WHERE valid_from IS NOT NULL;
