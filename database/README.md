Here we document the current state of the database. The history of these changes lives in the `sql` folder which contains all migrations. This document shows the schema and the purpose of the tables.

Code that directly interfaces with the database through SQL queries lives in the `database`. This crate is often wrapped into higher level components by consumers.

With a live database information for all tables can be retrieved with the `\d` command and information for a specific table with `\d MyTable`.

The database contains the following tables:

### auction\_participants
Summary:  
This table is used for [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f). It stores which solvers (identified by ethereum address) participated in which auctions (identified by auction id). CIP-20 specifies that "solver teams which consistently provide solutions" get rewarded.

   Column     |  Type  | Nullable | Details
--------------|--------|----------|--------
 auction\_id  | bigint | not null |
 participant  | bytea  | not null | <details>the solver that participated in the auction</details>

Indexes:  
- PRIMARY KEY: btree(`auction_id`, `participant`)

### auction\_prices

Summary:  
Stores the native price of a token in a given auction. Used for computations related to CIP-20.

 Column     | Type    | Nullable | Details
------------|---------|----------|--------
auction\_id | bigint  | not null | <details>in which auction this price was provided</details>
token       | bytea   | not null | <details>address of the token the price refers to</details>
price       | numeric | not null | <details>TODO</details>

Indexes:  
- PRIMARY KEY: btree(`auction_uid`, `token`)  

### auction\_transaction

Summary:  
Stores data required to recover the transaction with which a solver settled an auction.

 Coulmn      | Type   | Nullable | Details
-------------|--------|----------|--------
 auction\_id | bigint | not null | <details>the id of the auction</details>
 tx\_from    | bytea  | not null | <details>address of the solver account that won the auction</details>
 tx\_nonce   | bigint | not null | <details>nonce that will be used by the solver to settle the auction</details>

Indexes:  
- PRIMARY KEY: btree(`auction_id`)  

### auctions (and auctions\_id\_seq counter)

Summary:  
Stores only the current auction as a means to decouple auction creation in the `autopilot` from serving it in the `orderbook`. A new auction replaces the current one and uses the value of the `auctions_id_seq` sequence and increase it to ensure that auction ids are unique and monotonically increasing.  

 Column | Type   | Nullable | Details
--------|--------|----------|--------
 id     | bigint | not null | <details>other tables refer to this as auction\_id</details>
 json   | jsonb  | not null | <details>the serialized version of the auction. Technically the format is unspecified. The only requirement is that whatever format the `autopilot` stores can be parsed by the `orderbook`.</details>

Indexes:  
- PRIMARY KEY: btree(`id`)  

### ethflow\_orders

Summary:  
TODO try to understand why this needs to be like this

 Column    | Type   | Nullable | Details
-----------|--------|----------|--------
 uid       | bytea  | not null | <details>other tables refer to this as order\_uid</details>
 valid\_to | bigint | not null | <details>unix timestamp in seconds when the order expires</details>

Indexes:  
- PRIMARY KEY: btree(`uid`)  

### ethflow\_refunds

Summary:  
For orders buying some token with native ETH users temporarily transfer ownership of their ETH to the ethflow contract. When their order expires the `refunder` service automatically returns the ETH to the user. The table stores data about the transactions that refunded expired orders.  

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 order\_uid    | bytea  | not null | <details>the order that got refunded</details>
 block\_number | bigint | not null | <details>in which block the order got refunded</details>
 tx\_hash      | bytea  | not null | <details>the hash of the transaction that refunded the order</details>

Indexes:  
- PRIMARY KEY: btree(`order_uid`)  

### flyway\_schema\_history

Summary:  
We use flyway to do migrations of our database schema. This table contains metadata for flyway to know which and when migrations have been applied. Since this table only contains data managed by flyway and we didn't encounter any need to take a closer look at it we'll just refer to the [flyway docs](https://flywaydb.org/documentation/).

### interactions

Summary:  
The settlement contract allows associating user provided interactions to be executed before and after an order. This table stores these interactions and associates them with the respective orders.

 Column     | Type                   | Nullable | Details
------------|------------------------|----------|--------
 order\_uid | bytea                  | not null | <details>the order that this interaction belongs to</details>
 index      | integer                | not null | <details>index indicating in which interactions should be executed in case the same order has multiple interactions (ascending order)</details>
 target     | bytea                  | not null | <details>address of the smart contract this interaction should call</details>
 value      | numeric                | not null | <details>amount of ETH this interaction should send to the smart contract</details>
 data       | bytea                  | not null | <details>call data that contains the function selector and the bytes passed to it</details>
 execution  | [enum](#executiontime) | not null | <details>in which phase the interaction should be executed</details>

Indexes:  
- PRIMARY KEY: btree(`order_uid`)  


### invalidations

Summary:  
Stores data of [`OrderInvalidated`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L70-L71) events emited by [`invalidateOrder()`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L244-L255) of the settlement contract.

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 block\_number | bigint | not null | <details>the block in which the event was emitted</details>
 log\_index    | bigint | not null | <details>the index in which the log was emitted</details>
 order\_uid    | byteai | not null | <details>the order that got invalidated</details>

Indexes:  
- PRIMARY KEY: btree(`block_number, log_index`)  
- invalidations\_order\_uid: btree(`order_uid`, `block_number`, `log_index`)  

### onchain\_order\_invalidations

Summary:  
Stores data of [`OrderInvalidation`](https://github.com/cowprotocol/ethflowcontract/blob/main/src/interfaces/ICoWSwapOnchainOrders.sol#L46-L49) events emited by the `ICoWSwapOnchainOrders` interface.

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 block\_number | bigint | not null | <details>the block in which the event was emitted</details>
 log\_index    | bigint | not null | <details>the index in which the log was emitted</details>
 uid           | byteai | not null | <details>the order that got invalidated</details>

Indexes:  
- PRIMARY KEY: btree(`uid`)  
- invalidation\_event\_index: btree(`block_number, log_index`)  

### onchain\_placed\_orders

Summary:  
Stores data of [`OrderPlacement`](https://github.com/cowprotocol/ethflowcontract/blob/main/src/interfaces/ICoWSwapOnchainOrders.sol#L23-L44) events emited by the `ICoWSwapOnchainOrders` interface plus some metadata.

 Column           | Type                                | Nullable | Details
------------------|-------------------------------------|----------|--------
 uid              | bytea                               | not null | <details>the order that got created</details>
 sender           | bytea                               | not null | <details>the user that created the order with the smart contract</details>
 is\_reorged      | boolean                             | not null | <details>if the backend detects that an block creating an order got reorged it gets invalidated with this flag</details>
 block\_number    | bigint                              | not null | <details>the block in which the order was created</details>
 log\_index       | bigint                              | not null | <details>the index in which the `OrderPlacement` event was emitted </details>
 placement\_error | [enum](#onchainorderplacementerror) | nullable | <details>describes what error happened when placing the order</details>

Indexes:  
- PRIMARY KEY: btree(`uid`)  
- event\_index: btree(`block_number`, `index`)
- order\_sender: hash(sender)

### order\_execution

Summary:  
Contains metainformation for trades, required for reward computations that cannot be recovered from the blockchain and are not stored in a persistent manner somewhere else.

 Column       | Type    | Nullable | Details
--------------|---------|----------|--------
 order\_uid   | bytea   | not null | <details>which order this trade execution is related to</details>
 auction\_id  | bigint  | not null | <details>in which auction this trade was initiated</details>
 reward       | double  | not null | <details>revert adjusted solver rewards, deprecated in favor of [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f)</details>
 surplus\_fee | numeric | nullable | <details>dynamic fee computed by the protocol that should get taken from the surplus of a trade, this value only applies and is set for fill-or-kill limit orders.</details>
 solver\_fee  | numeric | nullable | <details>value that is used for objective value computations. This either contains a fee equal to the execution cost of this trade computed by a solver (only applies to partially fillable limit orders) or the solver\_fee computed by the backend adjusted for this trades fill amount (solver\_fees computed by the backend may include subsidies).</details>

Indexes:  
- PRIMARY KEY: btree(`order_uid`, `auction_id`)  
- order\_creation\_timestamp: btree(`creation_timestamp`)
- order\_owner: hash(`owner`)
- order\_quoting\_parameters: btree(`sell_token`, `buy_token`, `sell_amount`)
- order\_valid\_to: btree(`valid_to`)
- user\_order\_creation\_timestamp: btree(`owner`, `creation_timestamp` DESC)
- user\_valid\_to: btree(`valid_to`)
- version\_idx: btree(`settlement_contract`)

### order\_quotes

Summary:  
Quote data stored specifically for order that got created with the [`ICoWSwapOnchainOrders`](https://github.com/cowprotocol/ethflowcontract/blob/1d5d54a4ba890c5c0d3b26429ee32aa8e69f2f0d/src/interfaces/ICoWSwapOnchainOrders.sol#L6-L50) interface.  
TODO: verify  

 Colmun             | Type    | Nullable | Details
--------------------|---------|----------|--------
 order\_uid         | bytea   | not null | <details>the order that this quote belongs to</details>
 gas\_amount        | double  | not null | <details>the estimated gas used by the quote used to create this order with</details>
 gas\_price         | double  | not null | <details>the gas price at the time of order creation</details>
 sell\_token\_price | double  | not null | <details>the price of the sell\_token in ETH</details>
 sell\_amount       | numeric | not null | <details>the sell\_amount of the quote used to create the order with</details>
 buy\_amount        | numeric | not null | <details>the buy\_amount of the quote used to create the order with</details>

Indexes:  
- PRIMARY KEY: btree(`order_uid`)  

### orders

Summary:  
Contains all relevant signed data of an order and metadata that is important for correctly executing the order.

Column                    | Type                         | Nullable | Details
--------------------------|------------------------------|----------|--------
 uid                      | bytea                        | not null | <details>56 bytes identifier composed of a 32 bytes `hash` over the order data signed by the user, 20 bytes containing the `owner` and 4 bytes containing `valid_to`.</details>
 owner                    | bytea                        | not null | <details>where the sell\_token will be taken from</details>
 creation\_timestamp      | timestamptz                  | not null | <details>when the order was created</details>
 sell\_token              | bytea                        | not null | <details>address of the token that will be sold</details>
 buy\_token               | bytea                        | not null | <details>address of the token that will be bought</details>
 sell\_amount             | numeric                      | not null | <details>amount in sell\_token that should at most be sold</details>
 buy\_amount              | numeric                      | not null | <details>amount of buy\_token that should at least be bought</details>
 valid\_to                | timestamptz                  | not null | <details>point in time when the order can no longer be settled</details>
 fee\_amount              | numeric                      | not null | <details>amount in sell\_token the owner agreed upfront as a fee to be taken for the trade</details>
 kind                     | [enum](#orderkind)           | not null | <details>trade semantics of the order</details>
 partially\_fillable      | bool                         | not null | <details>determines if the order can be executed in multiple smaller trades or if everything has to be executed at once</details>
 signature                | bytea                        | not null | <details>signature provided by the owner stored as raw bytes. What these bytes mean is determined by signing\_scheme</details>
 cancellation\_timestamp  | timestamptz                  | nullable | <details>when the order was cancelled. If the the timestamp is null it means the order was not cancelled</details>
 receiver                 | bytea                        | nullable | <details>address that should receive the buy\_tokens. If this is null the owner will receive the buy tokens</details>
 app\_data                | bytea                        | not null | <details>Arbitrary data associated with this order but per design this is an IPFS hash which may contain additional meta data for this order signed by the user</details>
 signing\_scheme          | [enum](#signingscheme)       | not null | <details>what kind of signature was used to verify that the owner actually intended to create this order</details>
 settlement\_contract     | bytea                        | not null | <details>address of the contract that should be used to settle this order</details>
 sell\_token\_balance     | [enum](#selltokensource)     | not null | <details>defines how sell\_tokens need to be transferred into the settlement contract</details>
 buy\_token\_balance      | [enum](#buytokendestination) | not null | <details>defined how buy\_tokens need to be transferred back to the user</details>
 full\_fee\_amount        | numeric                      | not null | <details>estimation in sell\_token how much gas will be needed to execute this order</details>
 class                    | [enum](#orderclass)          | not null | <details>determines which special trade semantics will apply to the execution of this order</details>
 surplus\_fee             | numeric                      | nullable | <details>dynamic fee in sell\_token that gets regularly computed by the protocol for fill-or-kill limit orders, if this is null no surplus\_fee has been computed yet</details>
 surplus\_fee\_timestamp  | timestamptz                  | nullable | <details>when the surplus\_fee was computed for this order, the backend ignores orders with too old surplus\_fee\_timestamp because that order's surplus\_fee is too inaccurate</details>


Indexes:  
- PRIMARY KEY: btree(`uid`)

### presignature\_events

Summary:  
Stores data of [`PreSignature`](https://github.com/cowprotocol/contracts/blob/5e5c28877c1690415548de7bc4b5502f87e7f222/src/contracts/mixins/GPv2Signing.sol#L59-L61) events.


 Column        | Type    | Nullable | Details
---------------|---------|----------|--------
 block\_number | bigint  | not null | <details>the block in which the event was emitted</details>
 log\_index    | bigint  | not null | <details>the index in which the event was emitted</details>
 owner         | bytea   | not null | <details>the owner of the order</details>
 order\_uid    | bytea   | not null | <details>the order for which the signature was given or revoked</details>
 signed        | boolean | not null | <details>specifies if an a signature was given or revoked</details>

Indexes:  
- PRIMARY KEY: btreebtree(`block_number`, `log_index`)  
- most\_recent\_with\_orderuid: btree (`order_uid`, `block_number` DESC, `log_index` DESC)  
- presignature\_owner: hash(`owner`)

### quotes (and quotes\_id\_seq counter)

Summary:  
Stores quotes in order to determine whether it makes sense to allow a user to creat an order with a given `fee_amount`. Quotes are short lived and get removed when they expire. `id`s are unique and increase monotonically.

 Column                | Type               | Nullable | Details
-----------------------|--------------------|----------|--------
 sell\_token           | bytea              | not null | <details>address token that should be sold</details>
 sell\_amount          | numeric            | not null | <details>amount that should be sold at most</details>
 buy\_token            | bytea              | not null | <details>address of token that should be bought</details>
 buy\_amount           | numeric            | not null | <details>amount that should be bought at least</details>
 expiration\_timestamp | timestamptz        | not null | <details>when the quote should no longer considered valid. Invalid quotes will get deleted shortly</details>
 order\_kind           | [enum](#orderkind) | not null | <details>trade semantics for the quoted order</details>
 gas\_amount           | double             | not null | <details>amount of gas that would be used by the best quote</details>
 gas\_price            | double             | not null | <details>gas price at the time of quoting</details>
 sell\_token\_price    | double             | not null | <details>price of sell\_token in ETH. Since fees get taken in the sell token the actual fee will be computed with `sell_token_price * gas_amount * gas_used`.</details>
 id                    | bigint             | not null | <details>unique identifier of this quote</details>
 quote\_kind           | [enum](#quotekind) | not null | <details>semantics of the order the quote is generated for. Some orders cost more gas to execute since they incur some overhead. That needs to be reflected in a higher fee. When looking up a fee in the DB the order\_kind needs to match the order that the user wants to create.</details>

Indexes:  
- PRIMARY KEY: btree(`id`)  
- quotes\_token\_expiration: btree (`sell_token`, `buy_token`, `expiration_timestamp` DESC)  


### settlement\_observations

Summary:  
During the solver competition solvers promise a solution of a certain quality. If the settlement that eventually gets executed on-chain is worse than what was promised solvers can get slashed. This table stores the quality of the solution that was actually executed on-chain. (see [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f))

 Column                | Type    | Nullable | Details
-----------------------|---------|----------|--------
 block\_number         | bigint  | not null | <details>the block in which the settlement happened</details>
 log\_index            | bigint  | not null | <details>index of the [`Settlement`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L67-L68) event</details>
 gas\_used             | numeric | not null | <details>the amount of gas the settlement consumed</details>
 effective\_gas\_price | numeric | not null | <details>the effective gas price (basically the [EIP-1559](https://eips.ethereum.org/EIPS/eip-1559) gas price reduced to a single value)</details>
 surplus               | numeric | not null | <details>the amount of tokens users received above their limit price converted to ETH</details>
 fee                   | numeric | not null | <details>the total amount of `solver_fee` collected in the auction (see order\_execution.solver\_fee)</details>

Indexes:  
- PRIMARY KEY: btree(`block_number`, `log_index`)  

### settlement\_scores

Summary:  
Stores winning and follow up scores of every auction for [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f) reward computation.

 Column           | Type     | Nullable | Details
------------------|----------|----------|--------
 auction\_id      | bigint   | not null | <details>the id of the auction the scores belong to</details>
 winner           | bytea    | not null | <details>public address of the winning solver</details>
 winning\_score   | numeric  | not null | <details>the score the winning solver submitted. This is the quality the auction observed on-chain should achieve to not reesult in slasing of the solver.</details>
 reference\_score | numeric  | not null | <details>the score of the runner up solver. If only 1 solver submitted a valid solution this value is 0.</details>
 block\_deadline  | bigint   | not null | <details>the block at which the solver should have executed the solution at the latest before getting slashed for executing too slowly</details>

Indexes:  
- PRIMARY KEY: btree(`auction_id`)  

### settlements

Summary:  
Stores data and metadata of [`Settlement`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L67-L68) events emitted from the settlement contract.

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 block\_number | bigint | not null | <details>the block in which the settlement happened</details>
 log\_index    | bigint | not null | <details>the index in which the event was emitted</details>
 solver        | bytea  | not null | <details>the public address of the executing solver</details>
 tx\_hash      | bytea  | not null | <details>the transaction hash in which the settlement got executed</details>
 tx\_from      | bytea  | not null | <details>the address that submitted the transaction</details>
 tx\_nonce     | bigint | not null | <details>the nonce that was used to submit the transaction</details>

Indexes:  
- PRIMARY KEY: btree(`block_number`,`log_index`)  
- settlements\_tx\_from\_tx\_nonce: btree(`tx_from`, `tx_nonce`)
- settlements\_tx\_hash: hash(`tx_hash`)

### solver\_competitions

Summary:  
Stores an overview of the solver competition. It contains order contained in the auction along with prices for every relevant token as well as all valid solutions submitted by solvers together with their quality.

 Column | Type   | Nullable | Details
--------|--------|----------|--------
 id     | bigint | not null | <details>the id of the auction that the solver competition belongs to</details>
 json   | jsonb  | nullable | <details>overview of the solver competition with unspecified format</details>

Indexes:  
- PRIMARY KEY: btree(`id`)  

### trades

Summary:  
This table contains data of "trade" events issued by the settlement contract.
Trade events get issues for complete and for partial order executions.

 Column        | Type    | Nullable | Details
---------------|---------|----------|--------
 block\_number | bigint  | not null | <details>the block in which the event happened</details>
 log\_index    | bigint  | not null | <details>the index in which the event was emitted</details>
 order\_uid    | bytea   | not null | <details>executing a trade for this order caused the event to get emitted</details>
 sell\_amount  | numeric | not null | <details>the amount in sell\_token that got executed in this trade</details>
 buy\_amount   | numeric | not null | <details>the amount in buy\_token that got executed in this trade</details>
 fee\_amount   | numeric | not null | <details>the fee amount in sell\_token that got executed in this trade. Note that this amount refers to all or a portion of the static fee\_amount the user signed during the order creation.</details>

Indexes:  
- PRIMARY KEY: btree(`block_number`, `log_index`)  
- trade\_order\_uid: btree (`order_uid`, `block_number`, `log_index`)  

### Enums

#### executiontime

 Value | Meaning
 ------|--------
 pre   | interaction should be executed before sending tokens to the settlement contract
 post  | interaction should be executed after receiving bought tokens from the settlement contract

#### onchainorderplacementerror

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

#### orderkind

 Value | Meaning
-------|--------
 sell  | the order sells the entire sell\_amount for at least the user signed buy\_amount
 buy   | the order buys the entire buy\_amount for at most the user signed sell\_amount

#### signingscheme

 Value   | Meaning
---------|--------
 presign | TODO
 ethsign | TODO
 eip1271 | TODO
 eip712  | TODO

#### quotekind

 Value               | Meaning
---------------------|--------
 standard            | TODO
 eip1271onchainorder | TODO
 presignonchainorder | TODO

#### selltokensource

 Value    | Meaning
----------|--------
 erc20    | sell\_tokens will be drawn from the users regular ERC20 token allowance ([docs](https://docs.cow.fi/smart-contracts/vault-relayer/fallback-erc20-allowances))
 internal | sell\_tokens will be drawn from the balancer vault internal user balance ([docs](https://docs.cow.fi/smart-contracts/vault-relayer/balancer-internal-balances))
 external | sell\_tokens will be drawn from the user's ERC20 token balance but relayed through the balancer vault ([docs](https://docs.cow.fi/smart-contracts/vault-relayer/balancer-external-balances))

#### buytokendestination

 Value    | Meaning
----------|--------
 erc20    | Bought tokens will be added to the ERC20 token balance of that user
 internal | Bought tokens will be added to the balancer vault internal balance of the user ([docs](https://docs.cow.fi/smart-contracts/vault-relayer/balancer-internal-balances))

#### orderclass

 Value     | Meaning
-----------|--------
 market    | Short lived order that may receive surplus. Users agree to a static fee upfront by signing it.
 liquiidty | These orders must be traded at their limit price and may not receive any surplus. Violating this is a slashable offence.
 limit     | Long lived order that may receive surplus. Users sign a static fee of 0 upfront and either the backend or the solvers compute a dynamic fee that gets taken from the surplus (while still respecting the user's limit price!).

