-- Step 1: Add a new column to the quotes table
ALTER TABLE quotes 
    ADD COLUMN verified boolean;

-- Step 2: Add two new columns to the order_quotes table
ALTER TABLE order_quotes 
    ADD COLUMN verified boolean;

-- Step 3: Create table with quote interactions
CREATE TABLE quotes_interactions (
    quote_id bigint NOT NULL,
    index int NOT NULL,
    target bytea NOT NULL,
    value numeric(78,0) NOT NULL,
    call_data bytea,

    PRIMARY KEY (quote_id, index)
);

-- Get a specific quote's interactions.
CREATE INDEX quote_id_interactions ON quotes_interactions USING HASH (quote_id);
