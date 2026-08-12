-- Order and settlement tables, the solana.* counterparts of the EVM tables.
-- A row is final once its slot is at or below
-- solana.indexer_state.finalized_slot, the settlement-finalized NOTIFY fires
-- from that watermark's advance.
-- Runs on top of the base sql/ series in the same database: it reuses the
-- base OrderKind, OrderClass and ExecutionTime enums, so a solana database
-- applies the base series first.

CREATE TABLE solana.orders (
    uid                   bytea PRIMARY KEY CHECK (length(uid) = 32),
    owner                 bytea NOT NULL CHECK (length(owner) = 32),
    sell_token            bytea NOT NULL CHECK (length(sell_token) = 32),
    buy_token             bytea NOT NULL CHECK (length(buy_token) = 32),
    sell_token_account    bytea NOT NULL CHECK (length(sell_token_account) = 32),
    buy_token_account     bytea NOT NULL CHECK (length(buy_token_account) = 32),
    sell_amount           numeric(78,0) NOT NULL,
    buy_amount            numeric(78,0) NOT NULL,
    fee_amount            numeric(78,0) NOT NULL,
    -- Unix seconds, u32 on chain.
    valid_to              bigint NOT NULL,
    kind                  OrderKind NOT NULL,
    partially_fillable    boolean NOT NULL,
    app_data              bytea NOT NULL CHECK (length(app_data) = 32),
    intent_signature      bytea CHECK (length(intent_signature) = 64),
    -- Partially signed CreateOrder transaction bytes for gasless orders.
    presigned_transaction bytea,
    creation_timestamp    timestamp with time zone NOT NULL,
    -- market or limit, the liquidity class does not exist on Solana.
    class                 OrderClass NOT NULL,
    -- Canonical order PDA address, the reverse-lookup key during settlement
    -- decoding.
    order_pda             bytea NOT NULL CHECK (length(order_pda) = 32)
);

CREATE INDEX solana_orders_valid_to ON solana.orders (valid_to);
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
    amount_withdrawn       numeric(78,0) NOT NULL DEFAULT 0,
    amount_received        numeric(78,0) NOT NULL DEFAULT 0,
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
    settlement_tx_signature bytea NOT NULL,
    instruction_index       integer NOT NULL,
    -- '{}' = top-level, '{0,2,1}' = CPI path.
    inner_ix_path           integer[] NOT NULL DEFAULT '{}',
    order_uid               bytea NOT NULL CHECK (length(order_uid) = 32),
    -- Per-order pull from the BeginSettle instruction data, fee included.
    sell_amount             numeric(78,0) NOT NULL,
    -- Per-order push from the FinalizeSettle instruction data.
    buy_amount              numeric(78,0) NOT NULL,
    -- From the off-chain proposed-solution data.
    fee_amount              numeric(78,0) NOT NULL,
    PRIMARY KEY (settlement_tx_signature, instruction_index, inner_ix_path, order_uid),
    FOREIGN KEY (settlement_tx_signature, instruction_index)
        REFERENCES solana.settlements (tx_signature, instruction_index)
);

CREATE INDEX solana_trades_order_uid ON solana.trades (order_uid);

CREATE TYPE solana.account_meta AS (
    pubkey      bytea,
    is_signer   bool,
    is_writable bool
);

-- Pre/post interactions attached to an order, the solana.* counterpart of
-- the EVM interactions table.
CREATE TABLE solana.interactions (
    order_uid  bytea NOT NULL CHECK (length(order_uid) = 32),
    index      integer NOT NULL,
    execution  ExecutionTime NOT NULL,
    program_id bytea NOT NULL CHECK (length(program_id) = 32),
    accounts   solana.account_meta[] NOT NULL,
    data       bytea NOT NULL,
    PRIMARY KEY (order_uid, index, execution)
);

CREATE TABLE solana.order_quotes (
    order_uid            bytea PRIMARY KEY REFERENCES solana.orders(uid),
    -- Compute units.
    gas_amount           numeric(78,0) NOT NULL,
    -- Priority fee, lamports per compute unit.
    gas_price            numeric(78,0) NOT NULL,
    sell_token_price     numeric(78,0) NOT NULL,
    sell_amount          numeric(78,0) NOT NULL,
    buy_amount           numeric(78,0) NOT NULL,
    solver               bytea NOT NULL CHECK (length(solver) = 32),
    verified             boolean NOT NULL DEFAULT false,
    metadata             jsonb,
    creation_timestamp   timestamptz NOT NULL DEFAULT now(),
    expiration_timestamp timestamptz,
    -- Text on purpose: the EVM QuoteKind variants are signature-scheme
    -- specific.
    quote_kind           text NOT NULL DEFAULT 'standard'
);

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
    -- Grace-slots snapshot at window creation.
    finalization_grace_slots integer,
    -- 'unreconciled' marks rows reconstructed by the recovery scan.
    recovery_status          text,
    PRIMARY KEY (auction_id, solver, solution_uid)
);

