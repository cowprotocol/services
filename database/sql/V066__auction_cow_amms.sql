-- All cow amm owners that were part of an auction.
CREATE TABLE auction_cow_amms (
  auction_id bigint PRIMARY KEY,
  owners bytea[] NOT NULL
);
