-- The block height at which a sponsored order's presigned CreateOrder
-- transaction dies with its blockhash. Paired with the transaction bytes:
-- both set for a sponsored order, both absent otherwise.
ALTER TABLE solana.orders
    ADD COLUMN last_valid_block_height bigint,
    ADD CONSTRAINT solana_orders_sponsored_creation_paired CHECK (
        (presigned_transaction IS NULL) = (last_valid_block_height IS NULL)
    );
