-- Quotes handed out by the quote endpoint: the promised amounts, the solver
-- that promised them, and how long the promise holds.
CREATE TABLE solana.quotes (
    id                   bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    sell_token           bytea NOT NULL,
    buy_token            bytea NOT NULL,
    sell_amount          numeric(20,0) NOT NULL,
    buy_amount           numeric(20,0) NOT NULL,
    kind                 solana.OrderKind NOT NULL,
    solver               bytea NOT NULL,
    expiration_timestamp timestamptz NOT NULL
);
