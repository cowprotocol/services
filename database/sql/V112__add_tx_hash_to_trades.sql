-- Store the hash of the settlement transaction that contained each trade.
--
-- So far resolving a trade's transaction required joining the first settlement
-- event in the same block with a higher log index, which is awkward and easy
-- to get wrong when a block contains multiple settlements. Newly indexed
-- trades get the hash directly from the event log.
--
-- Nullable: rows indexed before this migration have to be backfilled manually
-- (long running update; run `settlement-finder backfill`, see
-- crates/settlement-finder).
ALTER TABLE trades
    ADD COLUMN tx_hash bytea;
