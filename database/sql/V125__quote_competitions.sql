-- Staging table for fast-path quote competitions. Written at quote time
-- with the serialized competition data; consumed by the autopilot's
-- fast-path handler which promotes the row into `competition_auctions`,
-- `proposed_solutions` and `proposed_trade_executions` using the real
-- `order_uid` before deleting the staging row.
CREATE TABLE quote_competitions (
    quote_id bigint PRIMARY KEY REFERENCES quotes(id) ON DELETE CASCADE,
    competition jsonb NOT NULL
);

-- Links a placed order back to the specific `quotes.id` (and, transitively,
-- `quote_competitions.quote_id`) the client committed to. Only set for
-- orders whose quote carried a fast-path `auction_id`.
ALTER TABLE order_quotes ADD COLUMN quote_id bigint;

-- Drop the `auction_id` columns V120 added as infrastructure for an earlier
-- design that hung competition data directly off the quote. The shipped
-- design keys competition data by `quote_id` (via `quote_competitions`) and
-- stores the synthetic `auction_id` inside the JSON blob, so both columns
-- are now unused.
ALTER TABLE quotes       DROP COLUMN auction_id;
ALTER TABLE order_quotes DROP COLUMN auction_id;
