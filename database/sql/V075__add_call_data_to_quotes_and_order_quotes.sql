-- This migration script is reversible.

-- Step 1: Add a new column to the quotes table
ALTER TABLE quotes 
    ADD COLUMN verified boolean;

-- Step 2: Add two new columns to the order_quotes table
ALTER TABLE order_quotes 
    ADD COLUMN verified boolean;

-- Step 3: Create table with quote interactions
CREATE TABLE quote_interactions (
    quote_id bigint NOT NULL,
    index int NOT NULL,
    target bytea NOT NULL,
    value numeric(78,0) NOT NULL,
    call_data bytea NOT NULL,

    PRIMARY KEY (quote_id, index),
    FOREIGN KEY (quote_id) REFERENCES quotes(id) ON DELETE CASCADE
);

-- Get a specific quote's interactions.
CREATE INDEX quote_id_interactions ON quote_interactions USING HASH (quote_id);

-- Step 4: Create table with quote interactions for order
CREATE TABLE order_quote_interactions (
    order_uid bytea NOT NULL,
    index int NOT NULL,
    target bytea NOT NULL,
    value numeric(78,0) NOT NULL,
    call_data bytea NOT NULL,

    PRIMARY KEY (order_uid, index)
);

-- Get a specific order's interactions.
CREATE INDEX order_uid_interactions ON order_quote_interactions USING HASH (order_uid);
