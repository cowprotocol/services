-- Quotes handed out by the quote endpoint: the promised amounts, the solver
-- that promised them, and how long the promise holds.
CREATE TABLE solana.quotes (
    id                   bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    sell_token           bytea NOT NULL CHECK (length(sell_token) = 32),
    buy_token            bytea NOT NULL CHECK (length(buy_token) = 32),
    sell_amount          numeric(20,0) NOT NULL,
    buy_amount           numeric(20,0) NOT NULL,
    kind                 solana.OrderKind NOT NULL,
    solver               bytea NOT NULL CHECK (length(solver) = 32),
    expiration_timestamp timestamptz NOT NULL
);
