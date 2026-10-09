-- The quote a fast-path auction settled; NULL for regular auctions.
ALTER TABLE competition_auctions ADD COLUMN fast_path_quote_id bigint;
