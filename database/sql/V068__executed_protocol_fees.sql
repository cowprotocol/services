-- Create a new type to store the executed protocol fee and the token this amount is paid in.
CREATE TYPE FeeAsset AS (
    amount numeric(78,0),
    token bytea
);

-- Add a new column to the order_execution table to store the vector of executed protocol fees.
-- Executed protocol fees are stored in the same ordering as the protocol fees in the fee_policies database table.
ALTER TABLE order_execution ADD COLUMN protocol_fees FeeAsset[] NOT NULL DEFAULT '{}';
