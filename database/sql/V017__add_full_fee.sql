-- 1. Add full fee column.
-- This column is a part of order metadata. Its values are used to settle orders;
-- they are only relevant for non-expired orders. Therefore, we can set the default
-- to `0`, deploy an intermediate version that stores this value but never actually
-- reads it, then wait till all the old orders are expired.
ALTER TABLE orders
    ADD COLUMN full_fee_amount numeric(78,0) NOT NULL DEFAULT 0;

-- 2. Drop defaults.
ALTER TABLE orders
    ALTER COLUMN full_fee_amount DROP DEFAULT;
