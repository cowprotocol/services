-- Order and settlement tables, the solana.* counterparts of the EVM tables.
-- Schema source: solana-services-specifications backend/database section 4,
-- adjusted to the finalized-watermark model: the per-row commitment columns
-- are gone, a row is finalized once its slot is at or below
-- solana.indexer_state.finalized_slot, and the settlement-finalized NOTIFY
-- fires from the watermark advance instead of a per-row commitment flip.
-- Reuses the OrderKind, OrderClass and ExecutionTime enums.

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
    -- u32 unix seconds on chain, wider here for safety.
    valid_to              bigint NOT NULL,
    kind                  OrderKind NOT NULL,
    partially_fillable    boolean NOT NULL,
    app_data              bytea NOT NULL CHECK (length(app_data) = 32),
    intent_signature      bytea CHECK (intent_signature IS NULL OR length(intent_signature) = 64),
    -- Partially signed CreateOrder transaction bytes for gasless orders,
    -- written by the orderbook at intake, read by the autopilot at submission.
    presigned_transaction bytea,
    creation_timestamp    timestamp with time zone NOT NULL,
    -- market | limit on Solana.
    class                 OrderClass NOT NULL,
    -- Canonical order PDA address, the reverse-lookup key during settlement
    -- decoding.
    order_pda             bytea NOT NULL CHECK (length(order_pda) = 32)
);

CREATE INDEX solana_orders_owner ON solana.orders USING hash (owner);
CREATE INDEX solana_orders_sell_buy_tokens ON solana.orders (sell_token, buy_token);
CREATE INDEX solana_orders_valid_to ON solana.orders (valid_to);
CREATE INDEX solana_orders_user_creation ON solana.orders (owner, creation_timestamp DESC);
CREATE UNIQUE INDEX solana_orders_order_pda ON solana.orders (order_pda);

-- Only orders the autopilot can act on wake the auction loop: off-chain
-- intents carry a signature, gasless ones a presigned transaction.
CREATE FUNCTION solana.notify_new_solana_order() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('solana_new_order', encode(NEW.uid, 'hex'));
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER solana_order_insert_notify
    AFTER INSERT ON solana.orders
    FOR EACH ROW
    WHEN (NEW.intent_signature IS NOT NULL OR NEW.presigned_transaction IS NOT NULL)
    EXECUTE FUNCTION solana.notify_new_solana_order();

-- Mutable on-chain order state, split from the immutable intent row above.
-- amount_withdrawn / amount_received are materialized running sums the
-- decoder maintains from TradeDelta events.
CREATE TABLE solana.order_pda (
    order_uid              bytea PRIMARY KEY REFERENCES solana.orders(uid),
    -- Rent payer of the order PDA.
    created_by             bytea NOT NULL CHECK (length(created_by) = 32),
    -- Owner of buy_token_account, resolved by the indexer.
    receiver_owner         bytea CHECK (receiver_owner IS NULL OR length(receiver_owner) = 32),
    amount_withdrawn       numeric(78,0) NOT NULL DEFAULT 0,
    amount_received        numeric(78,0) NOT NULL DEFAULT 0,
    -- NULL while the order PDA is live.
    cancellation_timestamp timestamp with time zone
);

CREATE INDEX solana_order_pda_receiver_owner
    ON solana.order_pda USING hash (receiver_owner) WHERE receiver_owner IS NOT NULL;
CREATE INDEX solana_order_pda_open
    ON solana.order_pda (order_uid) WHERE cancellation_timestamp IS NULL;

CREATE FUNCTION solana.notify_order_pda_changed() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('solana_order_pda_changed', encode(NEW.order_uid, 'hex'));
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER solana_order_pda_changed_notify
    AFTER INSERT OR UPDATE ON solana.order_pda
    FOR EACH ROW EXECUTE FUNCTION solana.notify_order_pda_changed();

-- One transaction can carry several settlements (one BeginSettle/
-- FinalizeSettle pair each), so the key includes the BeginSettle's top-level
-- instruction index.
CREATE TABLE solana.settlements (
    slot              bigint NOT NULL,
    tx_signature      bytea NOT NULL CHECK (length(tx_signature) = 64),
    instruction_index integer NOT NULL,
    solver            bytea NOT NULL CHECK (length(solver) = 32),
    auction_id        bigint NOT NULL,
    -- NULL only via the unmatched-attribution path; decode failures land in
    -- solana.dead_letter instead.
    solution_uid      bigint,
    PRIMARY KEY (tx_signature, instruction_index)
);

CREATE INDEX solana_settlements_auction_id ON solana.settlements (auction_id);
-- The finalization trigger below scans settlements by slot on every
-- watermark advance.
CREATE INDEX solana_settlements_slot ON solana.settlements (slot);

-- A settlement counts as finalized once the watermark passes its slot, so
-- the NOTIFY fires from the indexer_state.finalized_slot advance, covering
-- every settlement the advance newly finalized.
CREATE FUNCTION solana.notify_settlements_finalized() RETURNS trigger AS $$
DECLARE
    settlement record;
BEGIN
    FOR settlement IN
        SELECT auction_id, tx_signature FROM solana.settlements
        WHERE slot > OLD.finalized_slot AND slot <= NEW.finalized_slot
    LOOP
        PERFORM pg_notify(
            'solana_settlement_finalized',
            settlement.auction_id::text || ':' || encode(settlement.tx_signature, 'hex')
        );
    END LOOP;
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER solana_settlement_finalized_notify
    AFTER UPDATE OF finalized_slot ON solana.indexer_state
    FOR EACH ROW
    WHEN (NEW.finalized_slot > OLD.finalized_slot)
    EXECUTE FUNCTION solana.notify_settlements_finalized();

-- One row per TradeDelta: order_uid is part of the key because one
-- FinalizeSettle instruction emits one delta per affected order PDA.
CREATE TABLE solana.trades (
    settlement_tx_signature bytea NOT NULL,
    instruction_index       integer NOT NULL,
    -- '{}' = top-level; '{0,2,1}' = CPI path.
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

-- Covers fill-summary queries by order_uid without a heap fetch;
-- settlement_tx_signature is included as the join key to solana.settlements.
CREATE INDEX solana_trades_order_uid_cover
    ON solana.trades (order_uid)
    INCLUDE (buy_amount, sell_amount, fee_amount, settlement_tx_signature);

CREATE TYPE solana.account_meta AS (
    pubkey      bytea,
    is_signer   bool,
    is_writable bool
);

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
    -- 'landed' | 'rejected' | 'timeout'.
    outcome                  text,
    -- Signature from the indexer's settlement-finalized event, NULL until
    -- finalization.
    submitted_signature      bytea CHECK (submitted_signature IS NULL OR length(submitted_signature) = 64),
    -- Grace-slots snapshot at window creation.
    finalization_grace_slots integer,
    -- Provenance marker; only 'unreconciled' is written today.
    recovery_status          text,
    PRIMARY KEY (auction_id, solver, solution_uid)
);

CREATE INDEX solana_settlement_executions_time_range
    ON solana.settlement_executions (start_timestamp, end_timestamp);
