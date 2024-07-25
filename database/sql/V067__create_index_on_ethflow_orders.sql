-- To get all valid orders quickly, we create an index
CREATE INDEX ethflow_user_valid_to ON ethflow_orders USING BTREE (valid_to);
