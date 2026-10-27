-- `wait_for_cow` marks orders that hold back until a 15-minute slot (their
-- `valid_from` sits exactly on :00, :15, :30 or :45) so they can be batched
-- with other orders aiming at the same slot. Fast-path orders never qualify.
-- It is decided once at placement and only feeds fee rules. Existing orders
-- predate the classification, so `false` is the right value for all of them.
ALTER TABLE orders ADD COLUMN wait_for_cow boolean NOT NULL DEFAULT false;
