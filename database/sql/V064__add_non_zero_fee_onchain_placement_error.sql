-- Add a new error type for when onchain orders are placed with a non-zero fee.
ALTER TYPE OnchainOrderPlacementError ADD VALUE 'non_zero_fee';
