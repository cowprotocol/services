-- Add non-optional `simulation_block` field to settlement_scores table. 
ALTER TABLE settlement_scores ADD COLUMN simulation_block bigint NOT NULL DEFAULT 0;
ALTER TABLE settlement_scores ALTER COLUMN simulation_block DROP DEFAULT;

-- Winning settlement call data for each auction.
CREATE TABLE settlement_call_data (
  auction_id bigint PRIMARY KEY,
  call_data bytea NOT NULL,
  uninternalized_call_data bytea NOT NULL
);