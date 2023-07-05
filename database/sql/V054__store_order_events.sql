CREATE TYPE OrderEventLabel AS ENUM (
  'created',
  'ready',
  'filtered',
  'invalid',
  'executing',
  'considered',
  'traded',
  'cancelled'
);

-- This table stores timestamped events for every order to get per order metrics
-- for service level indicators (SLIs).
-- More info: https://github.com/cowprotocol/services/issues/1582
CREATE TABLE order_events (
    order_uid bytea NOT NULL,
    timestamp timestamptz NOT NULL,
    label OrderEventLabel NOT NULL
);

-- Add index on `order_uid` and `timestamp` to quickly get events for an order
-- (and a given time frame if required).
CREATE INDEX order_events_by_uid ON order_events USING BTREE (order_uid, timestamp);
