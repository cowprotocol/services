-- Notifies listeners of every new order event. The payload carries the whole
-- event so a listener never has to read `order_events` to forward it.
CREATE OR REPLACE FUNCTION notify_order_event()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify(
        'order_event',
        json_build_object(
            'uid', '0x' || encode(NEW.order_uid, 'hex'),
            'timestamp', NEW.timestamp,
            'label', NEW.label,
            'reason', NEW.reason
        )::text
    );
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER order_event_insert_notify
AFTER INSERT ON order_events
FOR EACH ROW
EXECUTE FUNCTION notify_order_event();
