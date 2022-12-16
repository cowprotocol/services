CREATE TYPE OnchainOrderPlacementError AS ENUM ('quote_id_not_found', 'not_allowed_buy_token', 'non_accepted_order_class', 'valid_to_too_far_in_future');

ALTER TABLE onchain_placed_orders ADD COLUMN placement_error OnchainOrderPlacementError DEFAULT NULL;
