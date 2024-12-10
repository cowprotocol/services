-- This script reverts changes applied in V075__add_metadata_to_quotes_and_order_quotes.sql migration script.

-- Step 1: Drop two columns from the quotes table
ALTER TABLE quotes
    DROP COLUMN verified,
    DROP COLUMN metadata;

-- Step 2: Drop two columns from the order_quotes table
ALTER TABLE order_quotes
    DROP COLUMN verified,
    DROP COLUMN metadata;
