-- Create a new table for persisting solver competition blobs.

CREATE TABLE solver_competitions
(
    id bigserial PRIMARY KEY,
    json jsonb
);
