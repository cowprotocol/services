CREATE TYPE PolicyKind AS ENUM ('surplus', 'volume');

CREATE TABLE fee_policies (
  auction_id bigint NOT NULL,
  order_uid bytea NOT NULL,
  -- The order in which the fee policies are inserted and applied.
  application_order SERIAL NOT NULL,

  -- The type of the fee policy.
  kind PolicyKind NOT NULL,
  -- The fee should be taken as a percentage of the surplus. The value is between 0 and 1.
  surplus_factor double precision,
  -- Cap the fee at a certain percentage of the order volume. The value is between 0 and 1.
  max_volume_factor double precision,
  -- The fee should be taken as a percentage of the order volume. The value is between 0 and 1.
  volume_factor double precision,

  PRIMARY KEY (auction_id, order_uid, application_order)
);