-- All surplus capturing JIT order owners that were part of an auction.
CREATE TABLE surplus_capturing_jit_order_owners (
  auction_id bigint PRIMARY KEY,
  owners bytea[] NOT NULL
);
