-- `wait_for_cow` marks orders that hold back until one of the predefined slots
-- (their `valid_from` starts exactly on a slot) so they can be batched with
-- other orders aiming at the same slot. Fast-path orders never qualify.
-- It is decided once at placement and only feeds fee rules. Existing orders
-- predate the classification, so `false` is the right value for all of them.
ALTER TABLE orders ADD COLUMN wait_for_cow boolean NOT NULL DEFAULT false;
