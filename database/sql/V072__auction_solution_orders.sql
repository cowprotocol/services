-- Table to store all proposed solutions for an auction, received from solvers during competition time.
-- A single auction can have multiple solutions, and each solution can contain multiple orders.

-- This design allows for multiple solutions from a single solver
CREATE TABLE auction_solutions (
   auction_id bigint NOT NULL,
   -- The block number until which the solutions should be settled
   -- Not NULL for winning orders
   deadline bigint,
   -- solver submission address
   solver bytea NOT NULL,
   -- Has to be unique accross auctions (hash of the (auction_id + solver_address + solutionId received from solver)?)
   solution_id numeric NOT NULL,

   PRIMARY (auction_id, solution_id)
);

-- For performant filtering of solutions by auction_id
CREATE INDEX idx_auction_id ON auction_solutions(auction_id);
-- For performant JOINs on solution_id
CREATE INDEX idx_solution_id_on_solution ON auction_solutions(solution_id);

-- Table to store all orders in a solution
CREATE TABLE auction_solution_orders (
   solution_id numeric NOT NULL,
   order_uid bytea NOT NULL,
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

   PRIMARY (solution_id, order_uid)
);

-- For performant JOINs on solution_id
CREATE INDEX idx_solution_id_on_order ON auction_solution_orders(solution_id);