-- Create index to improve `fetch_latest_token_price` query performance.
CREATE INDEX auction_prices_token_auction_id_idx ON auction_prices (token, auction_id DESC);