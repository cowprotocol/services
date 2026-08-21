-- Timestamped auction-progress events per order. Reuses the base
-- OrderEventLabel enum, like the V2 tables reuse OrderKind.
CREATE TABLE solana.order_events (
    order_uid bytea NOT NULL CHECK (length(order_uid) = 32),
    timestamp timestamptz NOT NULL,
    label OrderEventLabel NOT NULL
);

CREATE INDEX solana_order_events_by_uid ON solana.order_events USING BTREE (order_uid, timestamp);
