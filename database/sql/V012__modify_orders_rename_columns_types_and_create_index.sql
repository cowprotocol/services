ALTER TABLE orders
    RENAME COLUMN balance_from TO sell_token_balance;
ALTER TABLE orders
    RENAME COLUMN balance_to TO buy_token_balance;

ALTER TYPE BalanceFrom RENAME TO SellTokenSource;
ALTER TYPE BalanceTo RENAME TO BuyTokenDestination;

CREATE INDEX version_idx ON orders USING BTREE (settlement_contract);
