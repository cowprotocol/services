-- Adding new column on table quotes to differentiate between quoting methods
-- This is required as different quotes have different expirations
CREATE TYPE QuoteKind AS ENUM ('standard', 'eip1271onchainorder', 'presignonchainorder');

ALTER TABLE quotes
ADD COLUMN quote_kind QuoteKind Not NULL DEFAULT 'standard';
