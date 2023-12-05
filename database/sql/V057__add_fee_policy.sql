-- Fee polices represents input parameters for the fee calculation, where fee is taken to compensate the protocol for it's services.
-- There are different fee policies:
-- 1. Based on price improvement - if limit order is filled at the price higher than max(limit price, best_quote), then fee is taken as a cut from the price difference. For this type of fee policy, all parameters exist and the fee is minimum of them.
-- 2. Based on the volume - fee is taken as part of the order volume. For this type of fee policy, only `volume_percentage_factor` exist.
-- 3. Based on the absolute fee - fee is taken as an absolute value. For this type of fee policy, only `absolute_fee` exist.
CREATE TABLE fee_policies (
  auction_id bigint NOT NULL
  order_uid bytea NOT NULL
  -- The order in which the fee policies are inserted and applied.
  insertion_order SERIAL NOT NULL

  -- The fee should be taken as a percentage of the price improvement. The value is between 0 and 1.
  price_improvement_factor NUMERIC(3, 2) CHECK (price_improvement_factor >= 0 AND price_improvement_factor <= 1)
  -- The fee should be taken as a percentage of the order volume. The value is between 0 and 1.
  volume_factor NUMERIC(3, 2) CHECK (volume_factor >= 0 AND volume_factor <= 1)
  -- The fee should be taken as an absolute value.
  absolute_fee NUMERIC(78, 0)

  PRIMARY KEY (auction_id, order_uid)

  CONSTRAINT unique_auction_order UNIQUE (auction_id, order_uid, insertion_order)
);