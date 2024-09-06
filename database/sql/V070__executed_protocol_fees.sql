-- Executed protocol fees are stored in the same ordering as the protocol fees in the fee_policies database table.
-- protocol_fee_tokens and protocol_fee_amounts are arrays of the same length.
ALTER TABLE order_execution 
ADD COLUMN protocol_fee_tokens bytea[] NOT NULL DEFAULT '{}', 
ADD COLUMN protocol_fee_amounts numeric(78,0)[] NOT NULL DEFAULT '{}';

