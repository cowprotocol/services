-- Contains data for each settlement execution of an auction.
CREATE TABLE settlement_executions
(
    auction_id      integer     NOT NULL,
    solver          bytea       NOT NULL,
    start_timestamp timestamptz NOT NULL DEFAULT now(),
    end_timestamp   timestamptz,
    start_block     bigint      NOT NULL,
    end_block       bigint,
    deadline_block  bigint      NOT NULL,
    outcome         text,
    PRIMARY KEY (auction_id, solver)
);
