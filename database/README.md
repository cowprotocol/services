Here we document the current state of the database. The history of these changes lives in the `sql` folder which contains all migrations. This document shows the schema and the purpose of the tables.

Code that directly interfaces with the database through SQL queries lives in the `database`. This crate is often wrapped into higher level components by consumers.

With a live database information for all tables can be retrieved with the `\d` command and information for a specific table with `\d MyTable`.

The database contains the following tables:

### auction_participants

   Column    |  Type  | Nullable | Default
-------------|--------|----------|---------
 auction_id  | bigint | not null |
 participant | bytea  | not null |
Indexes:
- "auction_participants_pkey" PRIMARY KEY, btree (auction_id, participant)

This table is used for [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f). It stores which solvers (identified by ethereum address) participated in which auctions (identified by auction id). CIP-20 specifies that "solver teams which consistently provide solutions" get rewarded.


### auction_prices
### auction_transaction
### auctions (and auctions_id_seq counter))
### ethflow_orders
### ethflow_refunds
### fixed_bytes_test
### flyway_schema_history
### interactions
### invalidations
### onchain_order_invalidations
### onchain_placed_orders
### order_execution
### order_quotes
### orders
### presignature_events
### quotes (and quotes_id_seq counter)
### settlement_observations
### settlement_scores
### settlements
### solver_competitions
### trades
