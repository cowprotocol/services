-- Per-order penalty caps (CIP-87): the maximum penalty a solver can incur for
-- winning an order but failing to execute it, denominated in the native token.
--
-- Stored as an array mapped one-to-one with `order_uids` (like `price_values`
-- maps to `price_tokens`), so the solver accounting can compute penalties for
-- solutions that were not executed. NULL for auctions that predate this column
-- or were created while penalties were disabled.
ALTER TABLE competition_auctions
  ADD COLUMN penalty_caps_native numeric(78, 0) [];
