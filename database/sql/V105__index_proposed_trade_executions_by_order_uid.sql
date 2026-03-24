CREATE INDEX CONCURRENTLY IF NOT EXISTS proposed_trade_executions_by_order_uid
ON proposed_trade_executions (order_uid);
