-- Timestamped auction-progress events per order.
CREATE TYPE solana.OrderEventLabel AS ENUM (
  'created',
  'ready',
  'filtered',
  'invalid',
  'executing',
  'considered',
  'traded',
  'cancelled'
);

CREATE TABLE solana.order_events (
    order_uid bytea NOT NULL CHECK (length(order_uid) = 32),
    timestamp timestamptz NOT NULL,
    label solana.OrderEventLabel NOT NULL
);

CREATE INDEX solana_order_events_by_uid ON solana.order_events USING BTREE (order_uid, timestamp);
