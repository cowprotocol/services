ALTER TABLE order_execution
    ADD COLUMN block_number bigint NOT NULL;

-- Populate block_number for existing records
UPDATE order_execution
SET block_number = COALESCE(settlements.block_number, 0)
FROM settlements
WHERE order_execution.auction_id = settlements.auction_id;