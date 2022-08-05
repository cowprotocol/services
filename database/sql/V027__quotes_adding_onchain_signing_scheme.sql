-- Adding new column on table quotes to differentiate the two on-chain signing schemes
-- This is required as different signing schemes have different expirations
CREATE TYPE OnchainSigningScheme AS ENUM ('eip1271', 'presign');

ALTER TABLE quotes
ADD COLUMN onchain_signing_scheme OnchainSigningScheme DEFAULT NULL;
