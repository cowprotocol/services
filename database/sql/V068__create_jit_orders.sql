-- JIT orders are not stored in `orders` table but in a separate table. Two reasons for this:
-- 1. Some fields from `orders` table are not needed for JIT orders, such as `partially_fillable`, `settlement_contract` etc.
-- 2. JIT orders are observed from blockchain which means this table needs to be reorg safe, so it contains block_number and log_index.
CREATE TABLE jit_orders (
    block_number bigint NOT NULL,
    log_index bigint NOT NULL,
    uid bytea PRIMARY KEY,
    owner bytea NOT NULL,
    creation_timestamp timestamptz NOT NULL,
    sell_token bytea NOT NULL,
    buy_token bytea NOT NULL,
    sell_amount numeric(78,0) NOT NULL,
    buy_amount numeric(78,0) NOT NULL,
    valid_to bigint NOT NULL,
    app_data bytea NOT NULL,
    fee_amount numeric(78,0) NOT NULL,
    kind OrderKind NOT NULL,
    signature bytea NOT NULL, -- r + s + v
    receiver bytea,
    signing_scheme SigningScheme NOT NULL,
    sell_token_balance SellTokenSource NOT NULL,
    buy_token_balance BuyTokenDestination NOT NULL,
    UNIQUE (block_number, log_index)
);

-- Get a specific user's orders.
CREATE INDEX jit_order_owner ON jit_orders USING HASH (owner);

CREATE INDEX jit_order_creation_timestamp ON jit_orders USING BTREE (creation_timestamp);

-- To optimize the performance of the user_orders query, we introduce a new index that allows
-- us to quickly get the latest orders from a owner
CREATE INDEX jit_user_order_creation_timestamp ON jit_orders USING BTREE (owner, creation_timestamp DESC);

-- To optimize deletion of reorged orders
CREATE INDEX jit_event_id ON jit_orders USING BTREE (block_number, log_index);
