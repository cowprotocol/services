-- covering indexes to avoid IO when calculating the total surplus for users
CREATE INDEX CONCURRENTLY trades_covering ON trades (order_uid) INCLUDE (buy_amount, sell_amount, fee_amount);
CREATE INDEX CONCURRENTLY orders_owner_covering ON orders (owner) INCLUDE (uid, kind, buy_amount, sell_amount, fee_amount, buy_token, sell_token);
