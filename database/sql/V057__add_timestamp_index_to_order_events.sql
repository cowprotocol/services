-- Adds an index on the 'timestamp' column of 'order_events' table to facilitate efficient periodic cleanups.
CREATE INDEX order_events_by_timestamp ON order_events (timestamp);
