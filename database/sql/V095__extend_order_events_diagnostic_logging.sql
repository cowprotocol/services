-- Add new ENUM type for event classification
CREATE TYPE OrderEventType AS ENUM ('info', 'error');

-- Extend order_events table with diagnostic fields
ALTER TABLE order_events
    ADD COLUMN type OrderEventType,
    ADD COLUMN message TEXT,
    ADD COLUMN component TEXT;

-- Create index for querying diagnostic events by type
CREATE INDEX order_events_by_type ON order_events (type, timestamp)
    WHERE type IS NOT NULL;

-- Create index for querying events by component
CREATE INDEX order_events_by_component ON order_events (component, timestamp)
    WHERE component IS NOT NULL;
