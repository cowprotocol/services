-- `wait_for_cow` marks orders that are not fast-path and whose `valid_from`
-- starts exactly on one of the predefined slots. Clients can ask for it at
-- placement, in which case the backend moves `valid_from` onto a slot.
-- It is decided once at placement and only feeds fee rules. Existing orders
-- predate the classification, so `false` is the right value for all of them.
ALTER TABLE orders ADD COLUMN wait_for_cow boolean NOT NULL DEFAULT false;
