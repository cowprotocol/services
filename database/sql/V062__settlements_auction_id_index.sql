-- Optimization for when we JOIN settlements on auction_id.
CREATE INDEX settlements_auction_id ON settlements (auction_id);
