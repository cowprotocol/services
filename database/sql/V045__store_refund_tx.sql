CREATE TABLE ethflow_refunds
(
    order_uid bytea PRIMARY KEY,
    block_number bigint NOT NULL,
    tx_hash bytea NOT NULL
);

-- Ideally we would populate the ethflow_refunds table with the correct value for already refunded orders in the migration.
-- However, we don't have the required information in the DB so we simply use placeholder values to easily replace them
-- with the correct hash with a manual update.
INSERT INTO ethflow_refunds (order_uid, block_number, tx_hash)
SELECT uid, 0, '\x0000000000000000000000000000000000000000000000000000000000000000'
FROM ethflow_orders
WHERE is_refunded is true;

ALTER TABLE ethflow_orders DROP COLUMN is_refunded;
