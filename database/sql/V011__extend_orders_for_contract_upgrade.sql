CREATE TYPE BalanceFrom AS ENUM ('erc20', 'internal', 'external');
CREATE TYPE BalanceTo AS ENUM ('erc20', 'internal');

-- While we could have simply added columns, set them to not null and made the update values defaults,
-- This would mean that we will forever have to ensure we don't accidentally insert without specifying
-- these values explicitly. This is especially awkward for the settlement_contract, since the default
-- would be the old contract version. For this reason, we have chosen to go with the approach of
-- 1. Add columns, setting them not null with default values,
ALTER TABLE orders
    ADD COLUMN settlement_contract bytea NOT NULL default '\x3328f5f2cEcAF00a2443082B657CedEAf70bfAEf',
    ADD COLUMN balance_from BalanceFrom NOT NULL default 'erc20',
    ADD COLUMN balance_to BalanceTo NOT NULL default 'erc20';

-- 2. Drop defaults
ALTER TABLE orders
    ALTER COLUMN settlement_contract DROP DEFAULT,
    ALTER COLUMN balance_from DROP DEFAULT,
    ALTER COLUMN balance_to DROP DEFAULT;
