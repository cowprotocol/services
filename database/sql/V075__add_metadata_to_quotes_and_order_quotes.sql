-- This migration script is not reversible.

-- Step 1: Add two new columns to the quotes table 
ALTER TABLE quotes 
    ADD COLUMN verified boolean,
    ADD COLUMN metadata json;

-- Step 2: Update existing data with non-null values
UPDATE quotes SET verified = false, metadata = '{}'::json;

-- Step 3: Add NOT NULL constraint to newly added columns
ALTER TABLE quotes
    ALTER COLUMN verified SET NOT NULL,
    ALTER COLUMN metadata SET NOT NULL;


-- Step 4: Add two new columns to the order_quotes table
ALTER TABLE order_quotes 
    ADD COLUMN verified boolean,
    ADD COLUMN metadata json;

-- Step 5: Update existing data with non-null values
UPDATE order_quotes SET verified = false, metadata = '{}'::json;

-- Step 6: Add NOT NULL constraint to newly added columns
ALTER TABLE order_quotes
    ALTER COLUMN verified SET NOT NULL,
    ALTER COLUMN metadata SET NOT NULL;

