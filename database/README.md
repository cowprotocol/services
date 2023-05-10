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

Summary:  
Contains all relevant signed data of an order and metadata that is important for correctly executing the order.

Column                    | Type                 | Nullable | Default
--------------------------|----------------------|----------|--------
 uid                      | bytea                | not null |
 owner                    | bytea                | not null |
 creation\_timestamp      | timestamptz          | not null |
 sell\_token              | bytea                | not null |
 buy\_token               | bytea                | not null |
 sell\_amount             | numeric              | not null |
 buy\_amount              | numeric              | not null |
 valid\_to                | timestamptz          | not null |
 fee\_amount              | numeric              | not null |
 kind                     | orderkind            | not null |
 partially\_fillable      | bool                 | not null |
 signature                | bytea                | not null |
 cancellation\_timestamp  | timestamptz          | nullable |
 receiver                 | bytea                | nullable |
 app\_data                | bytea                | not null |
 signing\_scheme          | signingscheme        | not null |
 settlement\_contract     | bytea                | not null |
 sell\_token\_balance     | selltokensource      | not null |
 buy\_token\_balance      | buytokendestination  | not null |
 full\_fee\_amount        | numeric              | not null |
 class                    | orderclass           | not null |
 surplus\_fee             | numeric              | nullable |
 surplus\_fee\_timestamp  | timestamptz          | nullable |

Enum `orderkind`

 Value | Meaning
-------|--------
 sell  | sells the entire sell\_amount for at least the user signed buy\_amount
 buy   | buys the entire buy\_amount for at most the user signed sell\_amount

Enum `signingscheme`

 Value               | Meaning
---------------------|--------
 standard            | TODO
 eip1271onchainorder | TODO
 presignonchainorder | TODO

Enum `selltokensource`

 Value    | Meaning
----------|--------
 erc20    | TODO
 internal | TODO
 external | TODO

Enum `buytokendestination`

 Value    | Meaning
----------|--------
 erc20    | TODO
 internal | TODO

Enum `orderclass`

 Value     | Meaning
-----------|--------
 ordinary  | Short lived order that may receive surplus. Users agree to a static fee upfront by signing it. This can also be referred to as a market order.
 liquiidty | These orders must be traded at their limit price and may not receive any surplus. Violating this is a slashable offence.
 limit     | Long lived order that may receive surplus. Users sign a static fee of 0 upfront and either the backend or the solvers compute a dynamic fee that gets taken from the surplus (while still respecting the user's limit price!).

Indexes:  
- "orders\_pkey" PRIMARY KEY, btree (`uid`)

Description:  
 uid: 56 bytes identifier composed of a 32 bytes `hash` over the order data signed by the user, 20 bytes containing the `owner` and 4 bytes containing `valid_to`.  
 owner: where the sell\_token will be taken from  
 creation\_timestamp: when the order was created  
 sell\_token: address of the token that will be sold  
 buy\_token: address of the token that will be bought  
 sell\_amount: amount in sell\_token that should at most be sold  
 buy\_amount: amount of buy\_token that should at least be bought  
 valid\_to: point in time when the order can no longer be settled  
 fee\_amount: amount in sell\_token the owner agreed upfront as a fee to be taken for the trade  
 kind: whether the order is a buy or a sell order  
 partially\_fillable: determines if the order can be executed in multiple smaller trades or if it everything has to be executed at once  
 signature: signature provided by the owner stored as raw bytes. What these bytes mean is determined by signing\_scheme  
 cancellation\_timestamp: when the order was cancelled. If the the timestamp is null it means the order was not cancelled  
 receiver: address that should receive the buy\_tokens. If this is null the owner will receive the buy tokens  
 app\_data: Arbitrary data associated with this order but per design this is an IPFS hash which may contain additional meta data for this order signed by the user  
 signing\_scheme: what kind of signature was used to verify that the owner actually intended to create this order  
 settlement\_contract: address of the contract that should be used to settle this order  
 sell\_token\_balance: defines how sell\_tokens need to be transferred into the settlement contract. (see `selltokensource`)
 buy\_token\_balance: defined how buy\_tokens need to be transferred back to the user. (see `buytokendestination`)
 full\_fee\_amount: estimation in sell\_token how much gas will be needed to execute this order  
 class: determines which special trade semantics will apply to the execution of this order. See class enum for more information  
 surplus\_fee: dynamic fee in sell\_token that gets regularly computed by the protocol for fill-or-kill limit orders, if this is null no surplus\_fee has been computed yet  
 surplus\_fee\_timestamp: when the surplus\_fee was computed for this order, the backend ignores orders with too old surplus\_fee\_timestamp because that order's surplus\_fee is too inaccurate  


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
