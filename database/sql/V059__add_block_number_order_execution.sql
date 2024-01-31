ALTER TABLE order_execution
    ADD COLUMN block_number bigint NOT NULL DEFAULT 0;

-- Populate block_number for existing records
UPDATE order_execution
SET block_number = settlements.block_number
FROM settlements
WHERE order_execution.auction_id = settlements.auction_id;

ALTER TABLE order_execution
    ALTER COLUMN block_number DROP DEFAULT;