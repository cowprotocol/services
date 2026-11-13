-- The policy that repays the funder the creation fee of a sponsored order
-- from its first fill. The amount is in atoms of the order's surplus token:
-- the buy token of a sell order, the sell token of a buy order.
ALTER TYPE solana.PolicyKind ADD VALUE 'sponsoredcreation';

ALTER TABLE solana.fee_policies ADD COLUMN sponsored_creation_amount numeric(20, 0);
