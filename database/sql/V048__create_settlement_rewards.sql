-- Populated before settlement occurs on chain
-- (after the submissions have been ranked)
CREATE TABLE settlement_scores (
  auction_id bigint PRIMARY KEY,
  winning_score numeric(78,0) NOT NULL,
  reference_score numeric(78,0) NOT NULL
);

-- Populated after block finalization via transactionReceipt.
CREATE TABLE settlement_observations (
  -- block number and log index to uniquely `JOIN` on the `settlements`
  -- table, read from the transaction receipt
  block_number bigint NOT NULL,
  log_index bigint NOT NULL,
  gas_used numeric(78,0) NOT NULL,
  effective_gas_price numeric(78,0) NOT NULL,
  -- the surplus observed from the transaction call data,
  -- and converted to ETH with the auction external prices.
  surplus numeric(78,0) NOT NULL,
  fee numeric(78,0) NOT NULL,

  PRIMARY KEY (block_number, log_index)
);

CREATE TABLE auction_prices (
  auction_id bigint NOT NULL PRIMARY KEY,
  tokens bytea[] NOT NULL,
  prices numeric(78,0)[] NOT NULL
);

CREATE TABLE auction_participants (
 -- This links to the `auctions` table
 auction_id bigint PRIMARY KEY,
 -- All solvers who submitted a valid solution to the auction.
 participants bytea[]
);
