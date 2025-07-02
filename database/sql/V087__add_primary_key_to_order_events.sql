-- The `order_events` table needs a `REPLICA IDENTITY` in order for `DELETE`
-- operations to be published to replicas.
-- This migration adds a new `id` column as the `PRIMARY KEY` to serve as the
-- `REPLICA IDENTITY`.
ALTER TABLE order_events ADD COLUMN id BIGSERIAL PRIMARY KEY;
