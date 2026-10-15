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

-- Records whether the caller asked for fast-path treatment (`enableFastPath:
-- true` in app-data). The autopilot's fast-path handler is the sole owner
-- of `valid_from` once this flag is set: it either sets `valid_from = now()`
-- (feature disabled or limit-price check failed) or `now + exclusivity` and
-- initiates the fast-path settle. Legacy orders default to `false`.
ALTER TABLE orders ADD COLUMN fast_path boolean NOT NULL DEFAULT false;
