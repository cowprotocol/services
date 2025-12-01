-- Create a trigger function that notifies on new orders
CREATE OR REPLACE FUNCTION notify_new_order()
RETURNS trigger AS $$
BEGIN
    PERFORM pg_notify('new_order', '');
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

-- Create a trigger that fires after each insert on the orders table
CREATE TRIGGER order_insert_notify
AFTER INSERT ON orders
FOR EACH ROW
EXECUTE FUNCTION notify_new_order();
