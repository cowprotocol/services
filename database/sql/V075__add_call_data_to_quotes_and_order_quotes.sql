-- Step 1: Add a new column to the quotes table
ALTER TABLE quotes 
    ADD COLUMN call_data bytea,
    ADD COLUMN verified boolean;

-- Step 2: Add two new columns to the order_quotes table
ALTER TABLE order_quotes 
    ADD COLUMN call_data bytea,
    ADD COLUMN verified boolean;
