-- Serves the lookup of the auctions that executed one order.
CREATE INDEX solana_proposed_trade_executions_order_uid
    ON solana.proposed_trade_executions (order_uid);
