-- Creates new indexes required for the incremental solvable orders cache update process.
CREATE INDEX order_creation_cancellation ON orders USING BTREE (creation_timestamp, cancellation_timestamp);
CREATE INDEX order_execution_block_number ON order_execution USING BTREE (block_number);
CREATE INDEX ethflow_refunds_block_number ON ethflow_refunds USING BTREE (block_number);
