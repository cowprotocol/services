ALTER TABLE min_fee_measurements RENAME COLUMN token TO sell_token;

ALTER TABLE min_fee_measurements
  ADD COLUMN buy_token bytea,
  ADD COLUMN amount  numeric(78,0),
  ADD COLUMN order_kind OrderKind;
