CREATE TYPE PolicyKind AS ENUM ('priceimprovement', 'volume');

CREATE TABLE fee_policies (
  auction_id bigint NOT NULL,
  order_uid bytea NOT NULL,
  -- The order in which the fee policies are inserted and applied.
  application_order SERIAL NOT NULL,

  -- The type of the fee policy.
  kind PolicyKind NOT NULL,
  -- The fee should be taken as a percentage of the price improvement. The value is between 0 and 1.
  price_improvement_factor NUMERIC CHECK (price_improvement_factor >= 0 AND price_improvement_factor <= 1),
  -- The fee should be taken as a percentage of the order volume. The value is between 0 and 1.
  volume_factor NUMERIC CHECK (volume_factor >= 0 AND volume_factor <= 1),

  PRIMARY KEY (auction_id, order_uid, application_order)
);