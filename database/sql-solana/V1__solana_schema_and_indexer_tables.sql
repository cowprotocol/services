-- The solana.* namespace and the indexer-owned bookkeeping tables.
-- Schema source: solana-services-specifications backend/database sections 1 and 3,
-- on the finalized-watermark model: rows are finalized once their slot is at or
-- below solana.indexer_state.finalized_slot, there is no per-row commitment column.

CREATE SCHEMA solana;

-- Single-row watermark state. `slot` is the highest fully-processed slot the
-- stream resumes from, `finalized_slot` the highest slot the stream reported
-- finalized. Both are monotone non-decreasing; operator repair that must move
-- them backward deletes and re-inserts the row, bypassing the trigger.
CREATE TABLE solana.indexer_state (
    singleton      boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    slot           bigint NOT NULL,
    finalized_slot bigint NOT NULL DEFAULT 0
);

CREATE FUNCTION solana.indexer_state_monotone() RETURNS trigger AS $$
BEGIN
    IF NEW.slot < OLD.slot THEN
        RAISE EXCEPTION 'solana.indexer_state.slot is monotone non-decreasing; refusing % < %', NEW.slot, OLD.slot;
    END IF;
    IF NEW.finalized_slot < OLD.finalized_slot THEN
        RAISE EXCEPTION 'solana.indexer_state.finalized_slot is monotone non-decreasing; refusing % < %', NEW.finalized_slot, OLD.finalized_slot;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER indexer_state_monotone
    BEFORE UPDATE OF slot, finalized_slot ON solana.indexer_state
    FOR EACH ROW
    EXECUTE FUNCTION solana.indexer_state_monotone();

-- Last observed chain tip, written by the ingester on every slot (~400ms).
-- Unlogged: WAL overhead is unacceptable at that frequency, and a crash only
-- loses a value the live slot subscription re-derives. Tips can go backward
-- on reorgs and provider reconnects, so no monotone guard.
CREATE UNLOGGED TABLE solana.chain_tip (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    slot      bigint NOT NULL
);

-- Transactions the decoder could not decode, kept for replay by signature via
-- getTransaction. The unique index makes re-streamed payloads idempotent
-- (INSERT ON CONFLICT (tx_signature) DO NOTHING).
CREATE TABLE solana.dead_letter (
    slot         bigint NOT NULL,
    tx_signature bytea NOT NULL CHECK (length(tx_signature) = 64),
    reason       text NOT NULL,
    inserted_at  timestamp with time zone NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX solana_dead_letter_signature ON solana.dead_letter (tx_signature);
CREATE INDEX solana_dead_letter_inserted_at ON solana.dead_letter (inserted_at);

-- Slot ranges skipped wholesale (outage past the provider replay window).
-- Coarser than dead_letter: "no data for slots N..M at all", aimed at ops.
CREATE TABLE solana.lost_slot_ranges (
    from_slot   bigint PRIMARY KEY,
    to_slot     bigint NOT NULL,
    detected_at timestamp with time zone NOT NULL DEFAULT now(),
    reason      text NOT NULL,
    CONSTRAINT from_not_after_to CHECK (to_slot >= from_slot)
);

ALTER TABLE solana.lost_slot_ranges
    ADD CONSTRAINT solana_lost_slot_ranges_no_overlap
    EXCLUDE USING gist (int8range(from_slot, to_slot, '[]') WITH &&);

-- Append-only snapshot history of the settlement program state PDA (solver
-- allowlist + manager). The latest row (highest slot) is authoritative; the
-- autopilot re-reads it on LISTEN solana_settlement_state_pda_changed.
CREATE TABLE solana.settlement_state_pda (
    slot            bigint PRIMARY KEY,
    observed_at     timestamp with time zone NOT NULL DEFAULT now(),
    allowlist       bytea[] NOT NULL,
    manager         bytea NOT NULL CHECK (length(manager) = 32),
    pending_manager bytea CHECK (pending_manager IS NULL OR length(pending_manager) = 32)
);

CREATE FUNCTION solana.notify_settlement_state_pda_changed() RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('solana_settlement_state_pda_changed', '');
    RETURN NULL;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER solana_settlement_state_pda_changed_notify
    AFTER INSERT ON solana.settlement_state_pda
    FOR EACH ROW EXECUTE FUNCTION solana.notify_settlement_state_pda_changed();
