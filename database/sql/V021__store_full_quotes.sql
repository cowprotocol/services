-- This migration step will change how we store min-fee measurements to store
-- full quote data that can later be attached to an order.
--
-- This involves adding new columns for buy and sell amounts in the quotes as
-- well as renaming the tables so its more apparent what they are for.
-- Additionally, for the quotes table, we add a `bigserial` ID column for
-- generating unique quote IDs for each order.

ALTER TABLE min_fee_measurements RENAME TO quotes;
DROP INDEX min_fee_measurements_token_expiration;
DELETE FROM quotes;
ALTER TABLE quotes
    DROP COLUMN amount,
    ALTER COLUMN buy_token SET NOT NULL,
    ALTER COLUMN order_kind SET NOT NULL,
    ADD COLUMN id bigserial PRIMARY KEY,
    ADD COLUMN sell_amount numeric(78,0) NOT NULL,
    ADD COLUMN buy_amount numeric(78,0) NOT NULL;
CREATE INDEX quotes_token_expiration ON quotes USING BTREE
    (sell_token, buy_token, expiration_timestamp DESC);

ALTER TABLE order_fee_parameters RENAME TO order_quotes;
ALTER INDEX order_fee_parameters_pkey RENAME TO order_quotes_pkey;
ALTER TABLE order_quotes
    ADD COLUMN sell_amount numeric(78,0) NOT NULL DEFAULT 0,
    ADD COLUMN buy_amount numeric(78,0) NOT NULL DEFAULT 0;
ALTER TABLE order_quotes
    ALTER COLUMN sell_amount DROP DEFAULT,
    ALTER COLUMN buy_amount DROP DEFAULT;
