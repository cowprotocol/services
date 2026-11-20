-- A quote is the route its driver quoted with the cost apart, like the EVM
-- quotes table: sell_amount and buy_amount are the route's, network_fee is
-- in lamports, and the prices are the native prices the cost converts at. A
-- zero price marks a row stored without them, which no order is placed on.
ALTER TABLE solana.quotes
    ADD COLUMN network_fee      numeric(20,0) NOT NULL DEFAULT 0,
    ADD COLUMN sell_token_price numeric(20,0) NOT NULL DEFAULT 0,
    ADD COLUMN buy_token_price  numeric(20,0) NOT NULL DEFAULT 0,
    ADD COLUMN verified         boolean NOT NULL DEFAULT false;

-- The order's copy keeps the route amounts and the order's own cost: the
-- quote's network fee plus the rent of the order's buy token account, in
-- atoms of the token the order does not fix. The quote promises a sell order
-- buy_amount - execution_cost and charges a buy order
-- sell_amount + execution_cost.
ALTER TABLE solana.order_quotes
    ADD COLUMN execution_cost numeric(20,0) NOT NULL DEFAULT 0,
    ADD COLUMN verified       boolean NOT NULL DEFAULT false;
