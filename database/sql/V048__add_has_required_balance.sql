-- Add a flag to avoid quoting orders where the owner doesn't have the required balance.
-- To err on the safe side we initialize the flag to true per default.
ALTER TABLE orders ADD COLUMN has_sufficient_balance boolean NOT NULL DEFAULT true;
