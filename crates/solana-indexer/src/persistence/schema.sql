-- Test-only schema fixture for the solana-indexer PostgresStore tests.
--
-- This is NOT a committed flyway migration. It is applied to the docker
-- Postgres in the `#[ignore]` persistence tests only, so the `solana.*`
-- schema never lands on staging/prod while the shape is still moving. When it
-- settles, promote this to database/sql-solana/V001__*.sql plus a
-- `migrations-solana` compose service (mirroring database/sql-pool-indexer/).
--
-- Source of truth: database spec §4 (parallel-table schemas). Kept lean: the
-- autopilot-facing NOTIFY triggers are omitted (the store test does not need
-- them). The `orderkind` / `orderclass` enums are reused from the migrated
-- public schema (docker DB has the EVM migrations applied).

CREATE SCHEMA IF NOT EXISTS solana;

CREATE TABLE solana.orders (
    uid                bytea PRIMARY KEY CHECK (length(uid) = 32),
    owner              bytea NOT NULL CHECK (length(owner) = 32),
    sell_token         bytea NOT NULL CHECK (length(sell_token) = 32),
    buy_token          bytea NOT NULL CHECK (length(buy_token) = 32),
    sell_token_account bytea NOT NULL CHECK (length(sell_token_account) = 32),
    buy_token_account  bytea NOT NULL CHECK (length(buy_token_account) = 32),
    sell_amount        numeric(78,0) NOT NULL,
    buy_amount         numeric(78,0) NOT NULL,
    fee_amount         numeric(78,0) NOT NULL,
    valid_to           bigint NOT NULL,
    kind               orderkind NOT NULL,
    partially_fillable boolean NOT NULL,
    app_data           bytea NOT NULL CHECK (length(app_data) = 32),
    intent_signature   bytea CHECK (intent_signature IS NULL OR length(intent_signature) = 64),
    creation_timestamp timestamptz NOT NULL,
    class              orderclass NOT NULL,
    order_pda          bytea NOT NULL CHECK (length(order_pda) = 32)
);

CREATE TABLE solana.order_pda (
    order_uid              bytea PRIMARY KEY REFERENCES solana.orders(uid),
    created_by             bytea NOT NULL CHECK (length(created_by) = 32),
    receiver_owner         bytea CHECK (receiver_owner IS NULL OR length(receiver_owner) = 32),
    amount_withdrawn       numeric(78,0) NOT NULL DEFAULT 0,
    amount_received        numeric(78,0) NOT NULL DEFAULT 0,
    cancellation_timestamp timestamptz,
    commitment             text NOT NULL DEFAULT 'confirmed'
                           CHECK (commitment IN ('confirmed', 'finalized'))
);

CREATE TABLE solana.settlements (
    slot         bigint NOT NULL,
    tx_signature bytea PRIMARY KEY CHECK (length(tx_signature) = 64),
    solver       bytea NOT NULL CHECK (length(solver) = 32),
    auction_id   bigint NOT NULL,
    solution_uid bigint NULL,
    commitment   text NOT NULL DEFAULT 'confirmed'
                 CHECK (commitment IN ('confirmed', 'finalized'))
);

CREATE TABLE solana.trades (
    settlement_tx_signature bytea NOT NULL REFERENCES solana.settlements(tx_signature),
    instruction_index       integer NOT NULL,
    inner_ix_path           integer[] NOT NULL DEFAULT '{}',
    order_uid               bytea NOT NULL CHECK (length(order_uid) = 32),
    sell_amount             numeric(78,0) NOT NULL,
    buy_amount              numeric(78,0) NOT NULL,
    fee_amount              numeric(78,0) NOT NULL,
    commitment              text NOT NULL DEFAULT 'confirmed'
                            CHECK (commitment IN ('confirmed', 'finalized')),
    PRIMARY KEY (settlement_tx_signature, instruction_index, inner_ix_path, order_uid)
);

-- Slot watermark. Single row (id = 0). The real DDL lives in the database
-- schema spec; kept minimal here for the watermark round-trip test.
CREATE TABLE solana.indexer_state (
    id                 integer PRIMARY KEY DEFAULT 0 CHECK (id = 0),
    last_indexed_slot  bigint NOT NULL
);
