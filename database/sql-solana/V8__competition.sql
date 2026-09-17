-- The current auction, replaced on every cut. Its identity column is the
-- auction id sequence, so ids stay sequential across restarts like the EVM
-- auctions table.
CREATE TABLE solana.auctions (
    id       bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    tip_slot bigint NOT NULL,
    json     jsonb NOT NULL
);

-- Auctions that ran a competition, snapshot at ranking time.
CREATE TABLE solana.competition_auctions (
    id            bigint PRIMARY KEY,
    tip_slot      bigint NOT NULL,
    deadline_slot bigint NOT NULL,
    order_uids    bytea[] NOT NULL,
    price_tokens  bytea[] NOT NULL,
    price_values  numeric(20,0)[] NOT NULL
);

-- Every solution proposed during a competition. The autopilot generates
-- `uid` per auction, disambiguating the solver-assigned `id` across drivers.
-- The EVM twin also stores uniform clearing prices, the Solana solve wire
-- carries none.
CREATE TABLE solana.proposed_solutions (
    auction_id bigint NOT NULL,
    uid        bigint NOT NULL,
    id         bigint NOT NULL,
    solver     bytea NOT NULL CHECK (length(solver) = 32),
    is_winner  boolean NOT NULL,
    score      numeric(20,0) NOT NULL,
    PRIMARY KEY (auction_id, uid)
);

-- The order executions of every proposed solution.
CREATE TABLE solana.proposed_trade_executions (
    auction_id    bigint NOT NULL,
    solution_uid  bigint NOT NULL,
    order_uid     bytea NOT NULL CHECK (length(order_uid) = 32),
    executed_sell numeric(20,0) NOT NULL,
    executed_buy  numeric(20,0) NOT NULL,
    PRIMARY KEY (auction_id, solution_uid, order_uid)
);
