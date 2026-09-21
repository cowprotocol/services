-- The quote a sponsored order was placed against, copied at placement so the
-- record outlives any cleanup of solana.quotes, like the EVM order_quotes
-- table. The quote id stays as provenance.
CREATE TABLE solana.order_quotes (
    order_uid          bytea PRIMARY KEY CHECK (length(order_uid) = 32),
    quote_id           bigint NOT NULL,
    sell_amount        numeric(20,0) NOT NULL,
    buy_amount         numeric(20,0) NOT NULL,
    solver             bytea NOT NULL CHECK (length(solver) = 32),
    creation_timestamp timestamptz NOT NULL DEFAULT now()
);
