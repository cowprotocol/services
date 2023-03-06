-- Introduces tables needed for the calculation of solver rewards, based on the CIP20 proposal.
-- All solver participants submit a score, and the best score is selected as the winning score.
-- We also save the second best score, which is used as a reference score for the reward calculation.
-- Scores, prices and participants are populated before settlement occurs on chain. This means that 
-- they are populated for both successful and reverted settlements.
-- Observations are populated after settlement occurs on chain (only for successful settlements).

CREATE TABLE settlement_scores (
  auction_id bigint PRIMARY KEY,
  winner bytea NOT NULL,
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
  gas_used numeric(78,0) NOT NULL,
  effective_gas_price numeric(78,0) NOT NULL,
  -- the surplus observed from the transaction call data,
  -- and converted to ETH with the auction external prices.
  surplus numeric(78,0) NOT NULL,
  fee numeric(78,0) NOT NULL,

  PRIMARY KEY (block_number, log_index)
);

-- External prices for all tokens used in the winning solution.
CREATE TABLE auction_prices (
  auction_id bigint NOT NULL,
  token bytea NOT NULL,
  price numeric(78,0) NOT NULL,

  PRIMARY KEY (auction_id, token)
);

CREATE TABLE auction_participants (
  -- This links to the `auctions` table
  auction_id bigint NOT NULL,
  -- Solver who submitted a valid solution to the auction.
  participant bytea NOT NULL,

  PRIMARY KEY (auction_id, participant)
);
