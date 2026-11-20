-- What the driver took from a proposed fill, in the order's surplus token: the
-- execution cost, the solver fee and the protocol fee. NULL when the driver
-- reported none.
ALTER TABLE solana.proposed_trade_executions ADD COLUMN fee numeric(20,0);

-- What the order paid in the settlement, the execution cost, the solver fee and
-- the protocol fee, in the sell token. NULL when the winning proposal that
-- settled the trade reported no fee.
ALTER TABLE solana.order_execution ADD COLUMN executed_fee numeric(20,0);
