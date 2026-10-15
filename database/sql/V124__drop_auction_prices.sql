-- Drop the `auction_prices` table. Its data duplicated the `price_tokens` /
-- `price_values` arrays of `competition_auctions`, which every reader now uses.
DROP TABLE IF EXISTS auction_prices;
