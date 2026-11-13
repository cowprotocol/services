-- The canonical batch schedule the autopilot uses. Written by the autopilot
-- leader when it starts leading, so the orderbook can validate wait-for-CoW
-- orders and tell the frontend when the next canonical batch starts.
-- At most one row. No row means canonical batches are off.
CREATE TABLE canonical_batch_schedule (
    singleton boolean PRIMARY KEY DEFAULT true CHECK (singleton),
    interval_seconds integer NOT NULL CHECK (interval_seconds > 0),
    cutoff_seconds integer NOT NULL
        CHECK (cutoff_seconds >= 0 AND cutoff_seconds < interval_seconds),
    updated_at timestamptz NOT NULL
);
