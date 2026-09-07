-- Slot ranges the indexer could not recover: the stream's replay window was
-- exceeded and the RPC backfill failed. Rows in this range may be missing.
CREATE TABLE solana.lost_slot_ranges (
    -- Exclusive: the last slot indexed before the gap.
    from_slot    bigint NOT NULL,
    -- Inclusive: the tip observed when the gap was recorded.
    through_slot bigint NOT NULL,
    reason       text NOT NULL,
    recorded_at  timestamptz NOT NULL DEFAULT now()
);
