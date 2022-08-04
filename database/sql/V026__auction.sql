-- Stores the most recently created auction by the autopilot so that the api pods can read it.
CREATE TABLE auctions
(
    id bigserial PRIMARY KEY,
    json jsonb
);
