-- Per-order penalty caps (CIP-87): the maximum penalty a solver can incur for
-- winning an order but failing to execute it, denominated in the native token.
--
-- Stored for every order that appears in a ranked solution of the auction
-- (mirroring the scope of `fee_policies`), so the solver accounting can
-- compute penalties for solutions that were not executed.
CREATE TABLE order_penalty_caps (
  auction_id bigint NOT NULL,
  order_uid bytea NOT NULL,
  penalty_cap_native numeric(78, 0) NOT NULL,

  PRIMARY KEY (auction_id, order_uid)
);
