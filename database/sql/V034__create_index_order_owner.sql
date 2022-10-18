-- replace index for effective order searching
DROP INDEX public.user_order_creation_timestamp;

CREATE INDEX order_owner ON public.orders USING HASH (owner);
