ALTER TABLE ethflow_orders ADD COLUMN refund_tx bytea;
-- Ideally we would populate the `refund_tx` column with the correct value for already refunedn transactions in the migration.
-- However, we don't have the required information in the DB so we simply use the 0x00..00 hash as a sentinel value to easily
-- replace the placeholder values with the correct hash with a manual update.
UPDATE ethflow_orders SET refund_tx = '\x0000000000000000000000000000000000000000000000000000000000000000' WHERE is_refunded;
ALTER TABLE ethflow_orders DROP COLUMN is_refunded;
