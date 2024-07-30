-- JIT orders are stored in a separate table which contains data related only to JIT orders.
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
    --fee_amount numeric(78,0) NOT NULL,
    kind OrderKind NOT NULL,
    --partially_fillable boolean NOT NULL,
    signature bytea NOT NULL, -- r + s + v
    --cancellation_timestamp timestamptz
    receiver bytea,
    signing_scheme SigningScheme NOT NULL,
    --settlement_contract bytea NOT NULL
    sell_token_balance SellTokenSource NOT NULL,
    buy_token_balance BuyTokenDestination NOT NULL
    --full_fee_amount numeric(78,0) NOT NULL
    --class OrderClass NOT NULL
);

-- Get a specific user's orders.
CREATE INDEX jit_order_owner ON jit_orders USING HASH (owner);

CREATE INDEX jit_order_creation_timestamp ON jit_orders USING BTREE (creation_timestamp);

-- To optimize the performance of the user_orders query, we introduce a new index that allows
-- us to quickly get the latest orders from a owner
CREATE INDEX jit_user_order_creation_timestamp ON jit_orders USING BTREE (owner, creation_timestamp DESC);

-- To optimize deletion of reorged orders
CREATE INDEX jit_block_number ON jit_orders USING BTREE (block_number);
