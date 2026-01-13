CREATE INDEX orders_owner_live_limit 
ON orders USING btree (owner, confirmed_valid_to)
WHERE cancellation_timestamp IS NULL 
  AND class = 'limit';