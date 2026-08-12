-- The solana.* namespace and the indexer-owned bookkeeping tables.
-- Finality is a watermark: a row is final once its slot is at or below
-- solana.indexer_state.finalized_slot.

CREATE SCHEMA solana;

-- Single-row watermark state. `slot` is the highest fully-processed slot the
-- stream resumes from, `finalized_slot` the highest slot the stream reported
-- finalized. Both are monotone non-decreasing. Operator repair that must move
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

-- Transactions the decoder could not decode, kept for replay by signature.
-- The signature key dedupes re-streamed payloads.
CREATE TABLE solana.dead_letter (
    tx_signature bytea PRIMARY KEY CHECK (length(tx_signature) = 64),
    slot         bigint NOT NULL,
    reason       text NOT NULL,
    inserted_at  timestamp with time zone NOT NULL DEFAULT now()
);

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
-- allowlist + manager). The latest row (highest slot) is authoritative.
CREATE TABLE solana.settlement_state_pda (
    slot            bigint PRIMARY KEY,
    observed_at     timestamp with time zone NOT NULL DEFAULT now(),
    allowlist       bytea[] NOT NULL,
    manager         bytea NOT NULL CHECK (length(manager) = 32),
    pending_manager bytea CHECK (length(pending_manager) = 32)
);
