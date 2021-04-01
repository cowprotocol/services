CREATE TYPE SigningScheme AS ENUM ('eip712');

DELETE FROM orders;

ALTER TABLE orders
    DROP COLUMN app_data,
    ADD COLUMN receiver bytea,
    ADD COLUMN app_data bytea NOT NULL,
    ADD COLUMN signing_scheme SigningScheme NOT NULL;
