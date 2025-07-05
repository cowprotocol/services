-- The `order_events` table needs a `REPLICA IDENTITY` in order for `DELETE`
-- operations to be published to replicas.
-- This migration sets the replica identity to FULL, which uses the entire row
-- for identifying changes. This is a temporary solution until a proper primary key is added.

ALTER TABLE public.order_events REPLICA IDENTITY FULL;
