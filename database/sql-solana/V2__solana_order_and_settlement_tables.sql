-- Order and settlement tables, the solana.* counterparts of the EVM tables.
-- A row is final once its slot is at or below
-- solana.indexer_state.finalized_slot.
-- The series owns every type it uses, so it applies to a database without
-- the base sql/ series.

CREATE TYPE solana.OrderKind AS ENUM ('sell', 'buy');

CREATE TABLE solana.orders (
    uid                   bytea PRIMARY KEY CHECK (length(uid) = 32),
    owner                 bytea NOT NULL CHECK (length(owner) = 32),
    sell_token            bytea NOT NULL CHECK (length(sell_token) = 32),
    buy_token             bytea NOT NULL CHECK (length(buy_token) = 32),
    sell_token_account    bytea NOT NULL CHECK (length(sell_token_account) = 32),
    buy_token_account     bytea NOT NULL CHECK (length(buy_token_account) = 32),
    sell_amount           numeric(20,0) NOT NULL,
    buy_amount            numeric(20,0) NOT NULL,
    -- Unix seconds, u32 on chain.
    valid_to              bigint NOT NULL,
    -- Earliest unix second the order may enter an auction. NULL means no
    -- lower bound.
    valid_from            bigint,
    kind                  solana.OrderKind NOT NULL,
    partially_fillable    boolean NOT NULL,
    app_data              bytea NOT NULL CHECK (length(app_data) = 32),
    intent_signature      bytea CHECK (length(intent_signature) = 64),
    -- Partially signed CreateOrder transaction bytes for gasless orders.
    presigned_transaction bytea,
    creation_timestamp    timestamp with time zone NOT NULL,
    -- Canonical order PDA address, the reverse-lookup key during settlement
    -- decoding.
    order_pda             bytea NOT NULL CHECK (length(order_pda) = 32)
);

-- The btree/hash split and the column sets mirror the base orders indexes,
-- the queries they serve are the same.
CREATE INDEX solana_orders_valid_to ON solana.orders (valid_to);
CREATE INDEX solana_orders_valid_from ON solana.orders (valid_from)
    WHERE valid_from IS NOT NULL;
CREATE INDEX solana_orders_owner ON solana.orders USING hash (owner);
CREATE INDEX solana_orders_creation_timestamp ON solana.orders (creation_timestamp);
CREATE INDEX solana_orders_user_creation
    ON solana.orders (owner, creation_timestamp DESC);
CREATE INDEX solana_orders_sell_buy_tokens ON solana.orders (sell_token, buy_token);
CREATE INDEX solana_orders_quoting_parameters
    ON solana.orders (sell_token, buy_token, sell_amount);
CREATE UNIQUE INDEX solana_orders_order_pda ON solana.orders (order_pda);

-- Mutable on-chain order state, split from the immutable intent row above.
-- amount_withdrawn / amount_received are running sums the indexer folds
-- trades into.
-- No foreign key to solana.orders: the indexer records on-chain order
-- activity even when the intent row is not written yet.
CREATE TABLE solana.order_pda (
    order_uid              bytea PRIMARY KEY CHECK (length(order_uid) = 32),
    -- Rent payer of the order PDA.
    created_by             bytea NOT NULL CHECK (length(created_by) = 32),
    -- Owner of buy_token_account, resolved by the indexer.
    receiver_owner         bytea CHECK (length(receiver_owner) = 32),
    amount_withdrawn       numeric(20,0) NOT NULL DEFAULT 0,
    amount_received        numeric(20,0) NOT NULL DEFAULT 0,
    -- NULL while the order PDA is live.
    cancellation_timestamp timestamp with time zone
);

-- One transaction can carry several settlements (one BeginSettle/
-- FinalizeSettle pair each), so the key includes the BeginSettle's top-level
-- instruction index.
CREATE TABLE solana.settlements (
    slot              bigint NOT NULL,
    tx_signature      bytea NOT NULL CHECK (length(tx_signature) = 64),
    instruction_index integer NOT NULL,
    solver            bytea NOT NULL CHECK (length(solver) = 32),
    auction_id        bigint NOT NULL,
    -- NULL when the settlement matches no recorded competition solution.
    solution_uid      bigint,
    PRIMARY KEY (tx_signature, instruction_index)
);

CREATE INDEX solana_settlements_auction_id ON solana.settlements (auction_id);
-- Per-order accounting deltas of a settlement. order_uid completes the key
-- because one settlement moves several orders.
CREATE TABLE solana.trades (
    tx_signature            bytea NOT NULL,
    instruction_index       integer NOT NULL,
    order_uid               bytea NOT NULL CHECK (length(order_uid) = 32),
    -- Per-order pull from the BeginSettle instruction data, fee included.
    sell_amount             numeric(20,0) NOT NULL,
    -- Per-order push from the FinalizeSettle instruction data.
    buy_amount              numeric(20,0) NOT NULL,
    -- From the off-chain proposed-solution data.
    fee_amount              numeric(20,0) NOT NULL,
    PRIMARY KEY (tx_signature, instruction_index, order_uid)
);

CREATE INDEX solana_trades_order_uid ON solana.trades (order_uid);

-- The autopilot's settlement observation windows, one per dispatched winner.
CREATE TABLE solana.settlement_executions (
    auction_id               bigint NOT NULL,
    solver                   bytea NOT NULL CHECK (length(solver) = 32),
    solution_uid             bigint NOT NULL,
    start_timestamp          timestamptz NOT NULL,
    end_timestamp            timestamptz,
    start_slot               bigint NOT NULL,
    end_slot                 bigint,
    -- Driver-retry deadline slot.
    deadline_slot            bigint NOT NULL,
    -- NULL while the observation window is open.
    outcome                  text CHECK (outcome IN ('landed', 'rejected', 'timeout')),
    -- NULL until the settlement is finalized.
    submitted_signature      bytea CHECK (length(submitted_signature) = 64),
    PRIMARY KEY (auction_id, solver, solution_uid)
);

