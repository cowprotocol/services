-- Contains data for each settlement execution of an auction.
-- To check if the auction was settled on-chain, refer to the `settlements` table.
CREATE TABLE settlement_executions
(
    auction_id      bigint      NOT NULL,
    solver          bytea       NOT NULL,
    start_timestamp timestamptz NOT NULL,
    end_timestamp   timestamptz,
    start_block     bigint      NOT NULL,
    end_block       bigint,
    deadline_block  bigint      NOT NULL,
    outcome         text,
    PRIMARY KEY (auction_id, solver)
);
