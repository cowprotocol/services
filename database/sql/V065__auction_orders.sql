-- All orders that were part of an auction. An order can appear in multiple auctions.
CREATE TABLE auction_orders (
  auction_id bigint PRIMARY KEY,
  order_uids bytea[] NOT NULL
);
