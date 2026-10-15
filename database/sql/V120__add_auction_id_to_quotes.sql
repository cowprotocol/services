-- Links a stored quote to its solver competition ledger row in
-- `competition_auctions`. Nullable because only fast path quotes
-- need this link.
ALTER TABLE quotes ADD COLUMN auction_id bigint;

-- Copy of quotes.auction_id preserved when the quote is attached
-- to an order.
ALTER TABLE order_quotes ADD COLUMN auction_id bigint;
