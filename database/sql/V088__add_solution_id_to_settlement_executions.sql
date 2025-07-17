-- Clean up the table
DELETE FROM settlement_executions;

-- Add the new column `solution_uid`
ALTER TABLE settlement_executions
    ADD COLUMN solution_uid BIGINT NOT NULL;

-- Drop existing primary key
ALTER TABLE settlement_executions
    DROP CONSTRAINT settlement_executions_pkey;

-- Recreate primary key
ALTER TABLE settlement_executions
    ADD CONSTRAINT settlement_executions_pkey
        PRIMARY KEY (auction_id, solver, solution_uid);
