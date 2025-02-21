-- Contains data for each settlement execution of an auction.
CREATE TABLE settlement_executions
(
    auction_id      bigint      NOT NULL,
    solver          bytea       NOT NULL,
    start_timestamp timestamptz NOT NULL,
    end_timestamp   timestamptz,
    deadline_block  bigint      NOT NULL,
    outcome         text,
    PRIMARY KEY (auction_id, solver)
);
