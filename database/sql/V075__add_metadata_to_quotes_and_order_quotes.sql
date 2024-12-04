-- This migration script is reversible.

-- Step 1: Add two new columns to the quotes table
ALTER TABLE quotes 
    ADD COLUMN verified boolean,
    ADD COLUMN metadata json;

-- Step 2: Add two new columns to the order_quotes table
ALTER TABLE order_quotes 
    ADD COLUMN verified boolean,
    ADD COLUMN metadata json;
