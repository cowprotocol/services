-- Rename max_volume_factor column
ALTER TABLE fee_policies
    RENAME COLUMN max_volume_factor TO surplus_max_volume_factor;

-- Add `price_improvement` policy fee kind
ALTER TYPE PolicyKind ADD VALUE 'priceimprovement';

-- Add price improvement fee columns
ALTER TABLE fee_policies
    ADD COLUMN price_improvement_factor double precision,
    ADD COLUMN price_improvement_max_volume_factor double precision;
