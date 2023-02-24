-- Populated before settlement occurs on chain
-- (after the submissions have been ranked)
CREATE TABLE settlement_scores (
  auction_id bigint PRIMARY KEY,
  winning_score numeric(78,0) NOT NULL,
  -- The score from a runner-up solver, if there is one, otherwise zero.
  reference_score numeric(78,0) NOT NULL,
  -- winning solver has the obligation to settle the transaction onchain before the deadline
  block_deadline bigint NOT NULL
);

-- Populated after block finalization via transactionReceipt.
CREATE TABLE settlement_observations (
  -- block number and log index to uniquely `JOIN` on the `settlements`
  -- table, read from the transaction receipt
  block_number bigint NOT NULL,
  log_index bigint NOT NULL,
  gas_used numeric(78,0),
  effective_gas_price numeric(78,0),
  -- the surplus observed from the transaction call data,
  -- and converted to ETH with the auction external prices.
  surplus numeric(78,0),
  fee numeric(78,0),

  PRIMARY KEY (block_number, log_index)
);

CREATE TABLE auction_prices (
  auction_id bigint NOT NULL,
  token bytea NOT NULL,
  price numeric(78,0) NOT NULL
);

CREATE TABLE auction_participants (
 -- This links to the `auctions` table
 auction_id bigint PRIMARY KEY,
 -- All solvers who submitted a valid solution to the auction.
 participants bytea[]
);
