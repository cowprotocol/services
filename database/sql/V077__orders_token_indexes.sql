CREATE INDEX orders_sell_buy_tokens ON orders (sell_token, buy_token);

CREATE INDEX jit_orders_sell_buy_tokens ON jit_orders (sell_token, buy_token);
