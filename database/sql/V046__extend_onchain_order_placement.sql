CREATE TYPE OnchainOrderPlacementError AS ENUM ('quote_not_found', 'invalid_quote', 'pre_validation_error', 'disabled_order_class', 'valid_to_too_far_in_future', 'invalid_order_data', 'insufficient_fee', 'other' );

ALTER TABLE onchain_placed_orders ADD COLUMN placement_error OnchainOrderPlacementError DEFAULT NULL;
