-- All orders that were part of an Auction. An order can appear in multiple auctions.
CREATE TABLE auction_orders (
  auction_id bigint PRIMARY KEY,
  order_uid bytea NOT NULL,
);