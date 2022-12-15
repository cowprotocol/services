CREATE TABLE ethflow_refunds
(
    order_uid bytea PRIMARY KEY,
    block_number bigint NOT NULL,
    tx_hash bytea NOT NULL
);

-- Ideally we would populate the `refund_tx` column with the correct value for already refunedn transactions in the migration.
-- However, we don't have the required information in the DB so we simply use the 0x00..00 hash as a sentinel value to easily
-- replace the placeholder values with the correct hash with a manual update.
INSERT INTO ethflow_refunds (order_uid, block_number, tx_hash)
SELECT uid, 0, '\x0000000000000000000000000000000000000000000000000000000000000000'
FROM ethflow_orders
WHERE is_refunded is true;

ALTER TABLE ethflow_orders DROP COLUMN is_refunded;
