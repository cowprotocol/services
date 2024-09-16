-- Table to store all proposed solutions for an auction, received from solvers during competition time.
-- A single auction can have multiple solutions, and each solution can contain multiple orders.
-- Order is not allowed to be part of more than one solution within the same auction.

-- `solver` + `solution_id` are unique within an auction.

-- `deadline` is used to determine if the order is part of a winning solution.

CREATE TABLE auction_solution_orders (
   auction_id bigint NOT NULL,
   solver bytea NOT NULL,
   solution_id string NOT NULL,
   order_uid bytea NOT NULL,
   -- The block number until which the order should be settled.
   -- Not NULL for winning orders.
   deadline bigint,

   -- Order details
   sell_token bytea NOT NULL,
   buy_token bytea NOT NULL,
   limit_sell numeric(78,0) NOT NULL,
   limit_buy numeric(78,0) NOT NULL,
   side OrderKind NOT NULL,
   -- Uniform clearing price of the sell token
   sell_token_price numeric(78,0) NOT NULL,
   -- Uniform clearing price of the buy token
   buy_token_price numeric(78,0) NOT NULL,
   -- The effective amount that left the user's wallet including all fees.
   executed_sell numeric(78,0) NOT NULL,
   -- The effective amount the user received after all fees.
   executed_buy numeric(78,0) NOT NULL,

   PRIMARY (auction_id, solver, solution_id, order_uid)
);

CREATE INDEX idx_auction_id ON auction_solution_orders(auction_id);