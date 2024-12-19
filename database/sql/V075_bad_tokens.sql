CREATE TABLE bad_tokens (
    solver bytea NOT NULL,
    token bytea NOT NULL,
    heuristic_state jsonb,
    time_stamp timestamp,

    PRIMARY KEY (solver, token)
);