CREATE TYPE OrderFilterReason AS ENUM (
  'in_flight',
  'banned_user',
  'invalid_signature',
  'unsupported_token',
  'insufficient_balance',
  'dust_order',
  'missing_native_price'
);

ALTER TABLE order_events ADD COLUMN reason OrderFilterReason;
