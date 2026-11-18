-- Records whether the caller asked for the wait-for-CoW execution mode
-- (`enableWaitForCow: true` in app-data). Mutually exclusive with
-- `fast_path` and `valid_from`, which app-data validation enforces.
-- Legacy orders default to `false`.
ALTER TABLE orders ADD COLUMN wait_for_cow boolean NOT NULL DEFAULT false;
