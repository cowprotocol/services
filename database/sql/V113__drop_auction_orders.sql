-- Drop the redundant `auction_orders` junction table. Its data duplicated
-- `competition_auctions.order_uids`; the order->auction lookup now uses the GIN
-- index on that array (V112). The write path was removed in #4568, so this drop
-- ships one release later to ensure no pod running the old code still writes to
-- the table during the rollover.
DROP TABLE IF EXISTS auction_orders;
