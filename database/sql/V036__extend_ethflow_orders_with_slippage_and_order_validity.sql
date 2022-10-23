-- Defaults are selected, such that older orders are unlikely to be refunded
-- But mirgration is not relevant, as we do not have the ethflow orders live
ALTER TABLE ethflow_orders ADD slippage double precision NOT NULL DEFAULT 0;
ALTER TABLE ethflow_orders ADD validity_duration bigint NOT NULL DEFAULT 0;
