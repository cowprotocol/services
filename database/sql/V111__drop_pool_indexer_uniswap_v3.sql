-- Removes the pool-indexer tables from databases that run this shared
-- migration set. The indexer's schema (V110) was checked in here by mistake, 
-- so every DB's flyway run created these tables even though only the dedicated
-- `*_pool_indexer` DB ever uses them.
-- 
-- The indexer's own DB applies its schema from a separate migration location
-- (see `database/sql-pool-indexer/`) and never runs this.
-- Forward-only cleanup: V110 is left untouched so its checksum stays valid on
-- DBs that already applied it. Children (FK to uniswap_v3_pools) drop first.
DROP TABLE IF EXISTS uniswap_v3_pool_states;
DROP TABLE IF EXISTS uniswap_v3_ticks;
DROP TABLE IF EXISTS uniswap_v3_pools;
DROP TABLE IF EXISTS pool_indexer_checkpoints;
