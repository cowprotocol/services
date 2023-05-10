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

Summary:  
This table contains data of "trade" events issued by the settlement contract.
Trade events get issues for complete and for partial order executions.

 Column        | Type    | Nullable | Default
---------------|---------|----------|--------
 block\_number | bigint  | not null |
 log\_index    | bigint  | not null |
 order\_uid    | bytea   | not null |
 sell\_amount  | numeric | not null |
 buy\_amount   | numeric | not null |
 fee\_amount   | numeric | not null |

Indexes:  
- "trades\_pkey" PRIMARY KEY, btree (`block_number`, `log_index`)  
- "trade\_order\_uid" btree (`order_uid`, `block_number`, `log_index`)  

Description:  
- block\_number: the block in which the event happened  
- log\_index: the index in which the event was emitted  
- order\_uid: executing a trade for this order caused the event to get emitted  
- sell\_amount: the amount in sell\_token that got executed in this trade  
- buy\_amount: the amount in buy\_token that got executed in this trade  
- fee\_amount: the fee amount in sell\_token that got executed in this trade
  Note that this amount refers to all or a portion of the static fee\_amount the user signed during the order creation.  
