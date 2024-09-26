-- Rename current `auctions` table to `latest_auction` since it always contains only one and only the latest auction
ALTER TABLE auctions RENAME TO latest_auction;

CREATE TABLE auctions (
   auction_id bigint PRIMARY KEY,
   -- The block number at which the auction was created
   block bigint NOT NULL,
   -- The block number until which all winning solutions from a competition should be settled on-chain
   deadline bigint NOT NULL
);

-- Table to store all proposed solutions for an auction, received from solvers during competition time.
-- A single auction can have multiple solutions, and each solution can contain multiple order executions.
-- This design allows for multiple solutions from a single solver
CREATE TABLE proposed_solutions (
   auction_id bigint NOT NULL,
   -- Has to be unique within auction (hash of the (solver + solutionId received from solver)?)
   solution_id numeric NOT NULL,
   -- solver submission address
   solver bytea NOT NULL,
   -- Whether the solution is one of the winning solutions of the auction
   is_winner boolean NOT NULL,

   PRIMARY KEY (auction_id, solution_id)
);

-- Table to store all order executions of a solution
CREATE TABLE proposed_solution_executions (
   auction_id bigint NOT NULL,
   solution_id numeric NOT NULL,
   order_uid bytea NOT NULL,
   -- Uniform clearing price of the sell token
   sell_token_price numeric(78,0) NOT NULL,
   -- Uniform clearing price of the buy token
   buy_token_price numeric(78,0) NOT NULL,
   -- The effective amount that left the user's wallet including all fees.
   executed_sell numeric(78,0) NOT NULL,
   -- The effective amount the user received after all fees.
   executed_buy numeric(78,0) NOT NULL,

   PRIMARY KEY (auction_id, solution_id, order_uid)
);

-- Jit orders that were proposed by solvers during competition time, but not yet potentially executed
CREATE TABLE proposed_jit_orders (
   order_uid bytea PRIMARY KEY,
   sell_token bytea NOT NULL,
   buy_token bytea NOT NULL,
   limit_sell numeric(78,0) NOT NULL,
   limit_buy numeric(78,0) NOT NULL,
   side OrderKind NOT NULL
);