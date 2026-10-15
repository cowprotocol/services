-- The chain tip duplicated the last-indexed watermark: quiet slots advance
-- solana.indexer_state.slot every confirmed slot, so freshness checks read
-- that row and this table had no reader left.
DROP TABLE solana.chain_tip;
