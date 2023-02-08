-- Populated before settlement occurs on chain
-- (after the submissions have been ranked)
CREATE TABLE settlement_scores (
  settlement_id bytea NOT NULL,
  winning_score numeric(78,0) NOT NULL,
  reference_score numeric(78,0) NOT NULL
);

-- Populated after block finalization via transactionReceipt.
CREATE TABLE settlement_observations (
  -- the appended settlement ID read from the call data
  settlement_id bytea NOT NULL,
  -- the transaction hash and log index to be able to `JOIN` on the `settlement`
  -- table, read from the transaction receipt
  tx_hash bytea NOT NULL,
  log_index bigint NOT NULL,
  gas_used numeric(78,0) NOT NULL,
  effective_gas_price numeric(78,0) NOT NULL,
  -- the surplus observed from the transaction call data,
  -- and converted to ETH with the auction external prices.
  surplus numeric(78,0) NOT NULL,
  fee numeric(78,0) NOT NULL
);

CREATE TABLE auction_prices (
  auction_id bigint NOT NULL,
  token bytea NOT NULL,
  price numeric(78,0) NOT NULL
);
