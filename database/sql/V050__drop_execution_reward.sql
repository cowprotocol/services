-- No need to save the CIP14 reward in the database, since it is substituted with CIP20 scheme.

ALTER TABLE order_execution
DROP COLUMN reward;
