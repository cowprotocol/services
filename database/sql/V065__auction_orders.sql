-- All orders that were part of an Auction. An order can appear in multiple auctions.
CREATE TABLE auction_orders (
  auction_id bigint NOT NULL,
  order_uid bytea NOT NULL,

  PRIMARY KEY (auction_id, order_uid)
);
