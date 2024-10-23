-- Create new table to store highest block that was processed for a given
-- event source.
-- This is useful for indexing very rare events. So far we would store only
-- actually indexed events. On restarts we would look up the last stored
-- event and continue indexing from there. If there can be a week or more
-- between 2 events we could end up having to re-index 1 week worth of blocks
-- on every restart until a new event gets indexed which would move the
-- indexing "checkpoint" to the tip of the chain.
CREATE TABLE last_indexed_blocks (
    -- identifies which contract's event indexing progress the row tracks
    contract        TEXT PRIMARY KEY,
    -- last indexed block for the given contract
    block_number BIGINT NOT NULL
);

-- Populate new table for existing indexed events based on current logic
-- of determining the last processed block.
INSERT INTO last_indexed_blocks (contract, block_number)
VALUES ('settlements',
    GREATEST(
        (SELECT COALESCE(MAX(block_number), 0) FROM trades),
        (SELECT COALESCE(MAX(block_number), 0) FROM settlements),
        (SELECT COALESCE(MAX(block_number), 0) FROM invalidations),
        (SELECT COALESCE(MAX(block_number), 0) FROM presignature_events)
    )
);

INSERT INTO last_indexed_blocks (contract, block_number)
VALUES ('ethflow_refunds', (SELECT COALESCE(MAX(block_number), 0) FROM ethflow_refunds));

INSERT INTO last_indexed_blocks (contract, block_number)
VALUES ('onchain_orders', (SELECT COALESCE(MAX(block_number), 0) FROM onchain_placed_orders));
