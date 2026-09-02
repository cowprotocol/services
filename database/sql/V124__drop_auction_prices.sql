-- Drop the `auction_prices` table. Its data duplicated the `price_tokens` /
-- `price_values` arrays of `competition_auctions`, which every reader now uses.
-- This also removes `auction_prices_pkey` and the orphaned
-- `auction_prices_token_auction_id_idx` from V079 -- ~240 GB together on
-- mainnet -- so V079 needs no separate drop.
-- Must not be applied before the release that removed the write path is fully
-- rolled out, or pods running the old code error while writing to a dropped
-- table.
DROP TABLE IF EXISTS auction_prices;
