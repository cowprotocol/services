Here we document the current state of the database. The history of these changes lives in the `sql` folder which contains all migrations. This document shows the schema and the purpose of the tables.

Code that directly interfaces with the database through SQL queries lives in the `database`. This crate is often wrapped into higher level components by consumers.

With a live database information for all tables can be retrieved with the `\d` command and information for a specific table with `\d MyTable`.

The database contains the following tables:

### auction\_participants

   Column     |  Type  | Nullable | Default
--------------|--------|----------|---------
 auction\_id  | bigint | not null |
 participant  | bytea  | not null |

Indexes:  
- "auction\_participants\_pkey" PRIMARY KEY, btree (`auction_id`, `participant`)

This table is used for [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f). It stores which solvers (identified by ethereum address) participated in which auctions (identified by auction id). CIP-20 specifies that "solver teams which consistently provide solutions" get rewarded.


### auction\_prices


Summary:  
Stores the native price of a token in a given auction. Used for computations related to CIP-20.

 Column     | Type    | Nullable | Default
------------|---------|----------|--------
auction\_id | bigint  | not null |
token       | bytea   | not null |
price       | numeric | not null |

Indexes:  
- "auction\_prices\_pkey" PRIMARY KEY, btree (`auction_uid`, `token`)  

Description:  
auction\_id: in which auction this price was provided  
token: address of the token the price refers to  
price: TODO


### auction\_transaction

Summary:  
Stores data required to recover the transaction with which a solver settled an auction.

 Coulmn      | Type   | Nullable | Default
-------------|--------|----------|--------
 auction\_id | bigint | not null |
 tx\_from    | bytea  | not null |
 tx\_nonce   | bigint | not null |

Indexes:  
- "auction\_transaction\_pkey" PRIMARY KEY, btree (`auction_id`)  

auction\_id: which auction was settled  
tx\_from: address of the solver account that won the auction  
tx\_nonce: nonce that will be used by the solver to settle the auction  

### auctions (and auctions\_id\_seq counter)

Summary:  
Stores only the current auction as a means to decouple auction creation in the `autopilot` from serving it in the `orderbook`. A new auction replaces the current one and uses the value of the `auctions_id_seq` sequence and increase it to ensure that auction ids are unique and monotonically increasing.  

 Column | Type   | Nullable | Default
--------|--------|----------|--------
 id     | bigint | not null |
 json   | jsonb  | not null |

Indexes:  
- "auctions\_pkey" PRIMARY KEY, btree (`id`)  

id: the id of the auction  
json: the serialized version of the auction. Technically the format is unspecified. The only requirement is that whatever format the `autopilot` stores can be parsed by the `orderbook`.  

### ethflow\_orders

Summary:  
TODO try to understand why this needs to be like this

 Column    | Type   | Nullable | Default
-----------|--------|----------|--------
 uid       | bytea  | not null |
 valid\_to | bigint | not null |

Indexes:  
- "ethflow\_orders\_pkey" PRIMARY KEY, btree (`uid`)  

Description:  
uid: the `order_uid` associated with the ethflow order  
valid\_to: unix timestamp in seconds when the order expires  

### ethflow\_refunds

Summary:  
For orders buying some token with native ETH users temporarily transfer ownership of their ETH to the ethflow contract. When their order expires the `refunder` service automatically returns the ETH to the user. The table stores data about the transactions that refunded expired orders.  

 Column        | Type   | Nullable | Default
---------------|--------|----------|--------
 order\_uid    | bytea  | not null |
 block\_number | bigint | not null |
 tx\_hash      | bytea  | not null |

Indexes:  
- "ethflow\_refunds\_pkey" PRIMARY KEY, btree (`order_uid`)  

order\_uid: id of the order that got refunded  
block\_number: in which block the order got refunded  
tx\_hash: the hash of the transaction that refunded the order  

### flyway\_schema\_history

Summary:  
We use flyway to do migrations of our database schema. This table contains metadata for flyway to know which and when migrations have been applied. Since this table only contains data managed by flyway and we didn't encounter any need to take a closer look at it we'll just refer to the [flyway docs](https://flywaydb.org/documentation/).

### interactions

Summary:  
The settlement contract allows associating user provided interactions to be executed before and after an order. This table stores these interactions and associates them with the respective orders.

 Column     | Type    | Nullable | Default
------------|---------|----------|--------
 order\_uid | bytea   | not null |
 index      | integer | not null |
 target     | bytea   | not null |
 value      | numeric | not null |
 data       | bytea   | not null |

Enum `executiontime`  

 Value | Meaning
 ------|--------
 pre   | interaction should be executed before sending tokens to the settlement contract
 post  | interaction should be executed after receiving bought tokens from the settlement contract

Indexes:  
- "interactions\_pkey" PRIMARY KEY, btree (`order_uid`)  

Description:  
order\_uid: the order that this interaction belongs to  
index: index indicating in which interactions should be executed in case the same order has multiple interactions (ascending order)
target: address of the smart contract this interaction should call
value: amount of ETH this interaction should send to the smart contract
data: call data that contains the function selector and the bytes passed to it
execution: determines in which phase the interaction should be executed (see enum `executiontime`)


### invalidations

Summary:  
Stores data of [`OrderInvalidated`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L70-L71) events emited by [`invalidateOrder()`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L244-L255) of the settlement contract.

 Column        | Type   | Nullable | Default
---------------|--------|----------|--------
 block\_number | bigint | not null |
 log\_index    | bigint | not null |
 order\_uid    | byteai | not null |

Indexes:  
- "invalidations\_pkey" PRIMARY KEY, btree (`block_number, log_index`)  

Description:  
block\_number: the block in which the event was emitted
log\_index: the index in which the log was emitted
order\_uid: the order that got invalidated


### onchain\_order\_invalidations

Summary:  
Stores data of [`OrderInvalidation`](https://github.com/cowprotocol/ethflowcontract/blob/main/src/interfaces/ICoWSwapOnchainOrders.sol#L46-L49) events emited by the `ICoWSwapOnchainOrders` interface.

 Column        | Type   | Nullable | Default
---------------|--------|----------|--------
 block\_number | bigint | not null |
 log\_index    | bigint | not null |
 uid           | byteai | not null |

Indexes:  
- "onchain\_order\_invalidations\_pkey" PRIMARY KEY, btree (`block_number, log_index`)  

Description:  
block\_number: the block in which the event was emitted
log\_index: the index in which the log was emitted
uid: the order that got invalidated


### onchain\_placed\_orders

Summary:  
Stores data of [`OrderPlacement`](https://github.com/cowprotocol/ethflowcontract/blob/main/src/interfaces/ICoWSwapOnchainOrders.sol#L23-L44) events emited by the `ICoWSwapOnchainOrders` interface plus some metadata.

 Column           | Type                       | Nullable | Default
------------------|----------------------------|----------|--------
 uid              | bytea                      | not null |
 sender           | bytea                      | not null |
 is\_reorged      | boolean                    | not null |
 block\_number    | bigint                     | not null |
 log\_index       | bigint                     | not null |
 placement\_error | onchainorderplacementerror | nullable |

Indexes:  
- "onchain\_placed\_orders\_pkey" PRIMARY KEY, btree (`uid`)  

Enum `onchainorderplacementerror`  

 Value                           | Meaning
---------------------------------|--------
 quote\_not\_found               | the order was created without first requesting a quote from the backend
 invalid\_quote                  | the associated quote does not apply to the order
 pre\_validation\_error          | TODO
 disabled\_order\_order\_class   | order was created with 
 valid\_to\_too\_far\_in\_future | TODO
 invalid\_order\_data            | TODO
 insufficient\_fee               | TODO
 other                           | some other error occurred

Description:  
uid: the order that got created  
sender: the user that created the order with the smart contract  
is\_reorged: if the backend detects that an block creating an order got reorged it gets invalidated with this flag  
block\_number: the block in which the order was created  
log\_index: the index in which the `OrderPlacement` event was emitted  
placement\_error: describes what error happened when placing the order (see `onchainorderplacementerror`)  

### order\_execution

Summary:  
Contains metainformation for trades, required for reward computations that cannot be recovered from the blockchain and are not stored in a persistent manner somewhere else.

 Column       | Type    | Nullable | Default
--------------|---------|----------|--------
 order\_uid   | bytea   | not null |
 auction\_id  | bigint  | not null |
 reward       | double  | not null |
 surplus\_fee | numeric | nullable |
 solver\_fee  | numeric | nullable |

Indexes:  
- "order\_rewards\_pkey" PRIMARY KEY, btree (`order_uid`, `auction_id`)  
  This table was originally called "order\_rewards" and renamed since then but the old index remained

Description:  
order\_uid: which order this trade execution is related to  
auction\_id: in which auction this trade was initiated  
reward: revert adjusted solver rewards, deprecated in favor of [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f)  
surplus\_fee: dynamic fee computed by the protocol that should get taken from the surplus of a trade, this value only applies and is set for fill-or-kill limit orders.  
solver\_fee: value that is used for objective value computations. This either contains a fee equal to the execution cost of this trade computed by a solver (only applies to partially fillable limit orders) or the solver\_fee computed by the backend adjusted for this trades fill amount (solver\_fees computed by the backend may include subsidies).  


### order\_quotes

Summary:  
Quote data stored specifically for order that got created with the [`ICoWSwapOnchainOrders`](https://github.com/cowprotocol/ethflowcontract/blob/1d5d54a4ba890c5c0d3b26429ee32aa8e69f2f0d/src/interfaces/ICoWSwapOnchainOrders.sol#L6-L50) interface.  
TODO: verify  

 Colmun             | Type    | Nullable | Default
--------------------|---------|----------|--------
 order\_uid         | bytea   | not null |
 gas\_amount        | double  | not null |
 gas\_price         | double  | not null |
 sell\_token\_price | double  | not null |
 sell\_amount       | numeric | not null |
 buy\_amount        | numeric | not null |

Indexes:  
- "order\_quotes\_pkey" PRIMARY KEY, btree (`order_uid`)  

Description:  
order\_uid: the order that this quote belongs to  
gas\_amount: the estimated gas used by the quote used to create this order with  
gas\_price: the gas price at the time of order creation  
sell\_token\_price: the price of the sell\_token in ETH  
sell\_amount: the sell\_amount of the quote used to create the order with  
buy\_amount: the buy\_amount of the quote used to create the order with  

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
 kind: trade semantics of the order (see `orderkind`)
 partially\_fillable: determines if the order can be executed in multiple smaller trades or if everything has to be executed at once  
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


### presignature\_events

Summary:  
Stores data of [`PreSignature`](https://github.com/cowprotocol/contracts/blob/5e5c28877c1690415548de7bc4b5502f87e7f222/src/contracts/mixins/GPv2Signing.sol#L59-L61) events.

 Column        | Type    | Nullable | Default
---------------|---------|----------|--------
 block\_number | bigint  | not null |
 log\_index    | bigint  | not null |
 owner         | bytea   | not null |
 order\_uid    | bytea   | not null |
 signed        | boolean | not null |

Indexes:  
- "presignature\_events\_pkey" PRIMARY KEY, btree (`block_number`, `log_index`)  

Description:  
block\_number: the block in which the event was emitted  
log\_index: the index in which the event was emitted  
owner: the owner of the order  
order\_uid: the order for which the signature was given or revoked  
signed: specifies if an a signature was given or revoked  

### quotes (and quotes\_id\_seq counter)

Summary:  
Stores quotes in order to determine whether it makes sense to allow a user to creat an order with a given `fee_amount`. Quotes are short lived and get removed when they expire. `id`s are unique and increase monotonically.

 Column                | Type        | Nullable | Default
-----------------------|-------------|----------|--------
 sell\_token           | bytea       | not null |
 sell\_amount          | numeric     | not null |
 buy\_token            | bytea       | not null |
 buy\_amount           | numeric     | not null |
 expiration\_timestamp | timestamptz | not null |
 order\_kind           | orderkind   | not null |
 gas\_amount           | double      | not null |
 gas\_price            | double      | not null |
 sell\_token\_price    | double      | not null |
 id                    | bigint      | not null | nextval('quotes\_id\_seq')
 quote\_kind           | quotekind   | not null |

Indexes:  
- "quotes\_pkey" PRIMARY KEY, btree (`id`)  
- "quotes\_token\_expiration", btree (`sell_token`, `buy_token`, `expiration_timestamp` DESC)  


Enum `quotekind`  

 Value               | Meaning
---------------------|--------
 standard            | TODO
 eip1271onchainorder | TODO
 presignonchainorder | TODO

Description:  
sell\_token: address token that should be sold  
sell\_amount: amount that should be sold at most  
buy\_token: address of token that should be bought  
buy\_amount: amount that should be bought at least  
expiration\_timestamp: when the quote should no longer considered valid. Invalid quotes will get deleted shortly  
order\_kind: trade semantics for the quoted order (see `orderkind`)  
gas\_amount: amount of gas that would be used by the best quote  
gas\_price: gas price at the time of quoting  
sell\_token\_price: price of sell\_token in ETH. Since fees get taken in the sell token the actual fee will be computed with `sell_token_price * gas_amount * gas_used`.  
id: unique identifier of this quote  
quote\_kind: semantics of the order the quote is generated for. Some orders cost more gas to execute since they incur some overhead. That needs to be reflected in a higher fee. When looking up a fee in the DB the order\_kind needs to match the order that the user wants to create (see `quotekind`).


### settlement\_observations

Summary:  
During the solver competition solvers promise a solution of a certain quality. If the settlement that eventually gets executed on-chain is worse than what was promised solvers can get slashed. This table stores the quality of the solution that was actually executed on-chain. (see [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f))

 Column                | Type    | Nullable | Default
-----------------------|---------|----------|--------
 block\_number         | bigint  | not null |
 log\_index            | bigint  | not null |
 gas\_used             | numeric | not null |
 effective\_gas\_price | numeric | not null |
 surplus               | numeric | not null |
 fee                   | numeric | not null |

Indexes:  
- "settlement\_observations\_pkey" PRIMARY KEY, btree (`block_number`, `log_index`)  

Description:  
block\_number: the block in which the settlement happened
log\_index: index of the [`Settlement`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L67-L68) event  
gas\_used: the amount of gas the settlement consumed  
effective\_gas\_price: the effective gas price (basically the [EIP-1559](https://eips.ethereum.org/EIPS/eip-1559) gas price reduced to a single value)  
surplus: the amount of tokens users received above their limit price converted to ETH  
fee: the total amount of `solver_fee` collected in the auction (see order\_execution.solver\_fee)  


### settlement\_scores

Summary:  
Stores winning and follow up scores of every auction for [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f) reward computation.

 Column           | Type     | Nullable | Default 
------------------|----------|----------|--------
 auction\_id      | bigint   | not null |
 winner           | bytea    | not null |
 winning\_score   | numeric  | not null |
 reference\_score | numeric  | not null |
 block\_deadline  | bigint   | not null |

Indexes:  
- "settlement\_scores\_pkey" PRIMARY KEY, btree (`auction_id`)  

Description:  
auction\_id: the id of the auction the scores belong to  
winner: public address of the winning solver  
winning\_score: the score the winning solver submitted. This is the quality the auction observed on-chain should achieve to not reesult in slasing of the solver.  
reference\_score: the score of the runner up solver. If only 1 solver submitted a valid solution this value is 0.  
block\_deadline: the block at which the solver should have executed the solution at the latest before getting slashed for executing too slowly  

### settlements

Summary:  
Stores data and metadata of `[Settlement](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L67-L68)` events emitted from the settlement contract.

 Column        | Type   | Nullable | Default
---------------|--------|----------|--------
 block\_number | bigint | not null |
 log\_index    | bigint | not null |
 solver        | bytea  | not null |
 tx\_hash      | bytea  | not null |
 tx\_from      | bytea  | not null |
 tx\_nonce     | bigint | not null |

Indexes:  
- "settlements\_pkey" PRIMARY KEY, btree (`block_number`,`log_index`)  

Description:  
block\_number: the block in which the settlement happened  
log\_index: the index in which the event was emitted  
solver: the public address of the executing solver  
tx\_hash: the transaction hash in which the settlement got executed  
tx\_from: the address that submitted the transaction  
tx\_nonce: the nonce that was used to submit the transaction  


### solver\_competitions

Summary:  
Stores an overview of the solver competition. It contains order contained in the auction along with prices for every relevant token as well as all valid solutions submitted by solvers together with their quality.

 Column | Type   | Nullable | Default
--------|--------|----------|--------
 id     | bigint | not null |
 json   | jsonb  | nullable |

Indexes:  
- "solver\_competitions\_pkey" PRIMARY KEY, btree (`id`)  

Description:  
id: the id of the auction that the solver competition belongs to  
json: overview of the solver competition with unspecified format  

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
