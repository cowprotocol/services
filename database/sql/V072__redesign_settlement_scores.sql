CREATE TABLE auction_winners (
  auction_id bigint PRIMARY KEY,
  winners bytea[] NOT NULL,
  winning_scores numeric(78,0)[] NOT NULL,
  reference_scores numeric(78,0)[] NOT NULL,
  block_deadline bigint NOT NULL,
  simulation_block bigint NOT NULL
);

-- Migrate data from the old table into the new table
INSERT INTO auction_winners (auction_id, winners, winning_scores, reference_scores, block_deadline, simulation_block)
SELECT
  auction_id,
  ARRAY[winner] AS winners,
  ARRAY[winning_score] AS winning_scores,
  ARRAY[reference_score] AS reference_scores,
  block_deadline,
  simulation_block
FROM settlement_scores;

-- Drop the old table
DROP TABLE settlement_scores;