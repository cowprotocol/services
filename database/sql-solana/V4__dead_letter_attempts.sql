-- When the replay last considered this transaction. The replay claims
-- least-recently-tried rows first, so a failing or re-parked row rotates to
-- the back instead of starving the queue. Defaulted on insert: a fresh row
-- joins the back of the rotation.
ALTER TABLE solana.dead_letter
    ADD COLUMN last_attempt_at timestamptz NOT NULL DEFAULT now();
