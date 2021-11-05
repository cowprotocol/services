-- Delete all existing fee measurements. Fee measurements are temporary anyway,
-- so we aren't loosing any important information. At worst, we will cause
-- some "insufficient fee errors" right when the migration happens.
DELETE FROM min_fee_measurements;

-- Now alter the table and instead of storing the minimum fee directly, store it
-- as a gas amount and token gas price.
ALTER TABLE min_fee_measurements 
  DROP COLUMN min_fee,
  ADD COLUMN gas_amount double precision NOT NULL,
  ADD COLUMN gas_price double precision NOT NULL,
  ADD COLUMN sell_token_price double precision NOT NULL;
