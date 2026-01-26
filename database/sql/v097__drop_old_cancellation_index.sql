-- drop index over (creation_timestamp, cancellation timestamp)
-- since it can only be used optimally for queries on the creation_timestamp
-- and a new index over the cancellation timestamp was created in the previous
-- migration
DROP INDEX IF EXISTS order_creation_cancellation;

