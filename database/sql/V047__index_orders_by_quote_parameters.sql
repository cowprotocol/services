-- Orders that have the same `sell_token`, `buy_token` and `sell_amount` can have their `surplus_fee`
-- updated based on the same underlying quote.
-- To update an order's `surplus_fee` more quickly based on those parameters we need an index across
-- all those columns.
CREATE INDEX order_quoting_parameters ON orders USING BTREE (sell_token, buy_token, sell_amount);
