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

CREATE TABLE order_events (
    order_uid bytea NOT NULL,
    timestamp timestamptz NOT NULL,
    label OrderEventLabel NOT NULL
);
