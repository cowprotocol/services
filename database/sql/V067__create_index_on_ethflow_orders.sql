-- To get all valid orders quickly, we create an index
CREATE INDEX ethflow_user_valid_to ON ethflow_orders USING BTREE (valid_to);

-- Remove wrongly added index in V031__create_ethflow_orders.sql
DROP INDEX user_valid_to;
