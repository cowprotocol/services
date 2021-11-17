-- Associate orders with the fee parameters they were evaluated against when they were created for
-- future debugging.
CREATE TABLE order_fee_parameters (
  order_uid bytea PRIMARY KEY,
  gas_amount double precision NOT NULL,
  gas_price double precision NOT NULL,
  sell_token_price double precision NOT NULL
);
