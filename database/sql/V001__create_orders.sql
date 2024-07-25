-- `u256`s are stored as `numeric(78,0)` which is an integer with up to 78 decimal digits.
-- This is the number of digits in `2**256 - 1`.
-- Bytes are stored in `bytea` which is a variable size byte string. There is no way to specify a
-- fixed size.
-- `u32`s are stored in `bigint` which is an 8 bytes signed integer because Postgre does not have
-- unsigned integers.

CREATE TYPE OrderKind AS ENUM ('buy', 'sell');

CREATE TABLE orders (
    uid bytea PRIMARY KEY,
    owner bytea NOT NULL,
    creation_timestamp timestamptz NOT NULL,
    sell_token bytea NOT NULL,
    buy_token bytea NOT NULL,
    sell_amount numeric(78,0) NOT NULL,
    buy_amount numeric(78,0) NOT NULL,
    valid_to bigint NOT NULL,
    app_data bigint NOT NULL,
    fee_amount numeric(78,0) NOT NULL,
    kind OrderKind NOT NULL,
    partially_fillable boolean NOT NULL,
    signature bytea NOT NULL -- r + s + v
);

-- Get a specific user's orders.
CREATE INDEX order_owner ON orders USING HASH (owner);

-- Get all valid orders.
CREATE INDEX order_valid_to ON orders USING BTREE (valid_to);
