ALTER TABLE ethflow_orders ADD is_refunded boolean NOT NULL DEFAULT false;
ALTER TABLE ethflow_orders ADD sufficient_slippage boolean NOT NULL DEFAULT false;
ALTER TABLE ethflow_orders ADD validity_duration bigint NOT NULL;
