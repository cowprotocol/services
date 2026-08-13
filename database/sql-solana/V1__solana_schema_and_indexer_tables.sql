-- The solana.* namespace and the indexer-owned bookkeeping tables.
-- A row is final once its slot is at or below
-- solana.indexer_state.finalized_slot.

CREATE SCHEMA solana;

-- Single-row indexer progress. `slot` is the last fully indexed slot, the
-- stream resumes one past it. `finalized_slot` is the highest slot the
-- stream reported finalized. Both are monotone non-decreasing. Operator repair that must move
-- them backward deletes and re-inserts the row, bypassing the trigger.
CREATE TABLE solana.indexer_state (
    singleton      boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    slot           bigint NOT NULL,
    finalized_slot bigint NOT NULL DEFAULT 0
);

-- Last observed chain tip, written by the ingester on every slot (~400ms).
-- Separate from indexer_state: the tip streams before the first flush writes
-- that row, and reorgs move it backward, so it shares no monotone guarantee.
CREATE TABLE solana.chain_tip (
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
