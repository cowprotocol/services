-- When the replay last tried this transaction. NULL means never tried, and
-- the replay claims least-recently-tried rows first, so a permanently
-- failing row rotates to the back instead of starving the queue.
ALTER TABLE solana.dead_letter ADD COLUMN last_attempt_at timestamptz;
