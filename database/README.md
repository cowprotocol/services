Here we document the current state of the database. The history of these changes lives in the `sql` folder which contains all migrations. This document shows the schema and the purpose of the tables.

Code that directly interfaces with the database through SQL queries lives in the `database`. This crate is often wrapped into higher level components by consumers.

With a live database information for all tables can be retrieved with the `\d` command and information for a specific table with `\d MyTable`.

Some tables only store data emitted via smart contract events. Because we only have a single deployment of the [`GPv2Settlement`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol) settlement contract shared across staging and production environments events related to staging **and** production orders and settlements will be present in **both** the staging **and** production databases.
[CoWSwapEthFlow](https://github.com/cowprotocol/ethflowcontract/blob/main/src/CoWSwapEthFlow.sol) we actually deployed twice so events related to the staging environment should only show up in the staging DB and likewise for production.
It's also important to note that we only index events from blocks that we are certain will not get reorged. That means specifically that events will be indexed with a block delay of at least 64.

### app\_data

Associates the 32 bytes contract app data with the corresponding full app data.

See [here](https://github.com/cowprotocol/services/issues/1465) for more details. In this table the contract app data is either the old unixfs based scheme, or the new keccak scheme. The new scheme can be validated by keccak-256 hashing the full app data, which should produce the contract app data. The old scheme cannot be validated.

Column               | Type  | Nullable | Details
---------------------|-------|----------|-------
 contract\_app\_data | bytea | not null | 32 bytes. Referenced by `orders.app_data`.
 full\_app\_data     | bytea | not null | Is utf-8 but not stored as string because the raw bytes are important for hashing.

Indexes:
- "app\_data\_pkey" PRIMARY KEY, btree (`contract_app_data`)

### auction\_participants

This table is used for [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f). It stores which solvers (identified by ethereum address) participated in which auctions (identified by auction id). CIP-20 specifies that "solver teams which consistently provide solutions" get rewarded.

   Column     |  Type  | Nullable | Details
--------------|--------|----------|--------
 auction\_id  | bigint | not null | id of the auction
 participant  | bytea  | not null | solver that submitted a **valid** solution for the auction

Indexes:
- PRIMARY KEY: btree(`auction_id`, `participant`)

### auction\_prices

Stores the native price of a token in a given auction. Used for computations related to CIP-20.

 Column     | Type    | Nullable | Details
------------|---------|----------|--------
auction\_id | bigint  | not null | in which auction this price was provided
token       | bytea   | not null | address of the token the price refers to
price       | numeric | not null | the atoms of ETH that can be bought with 1 atom of the token

Indexes:
- PRIMARY KEY: btree(`auction_uid`, `token`)

### auctions (and auctions\_id\_seq counter)

Contains only the current auction to decouple auction creation in the `autopilot` from serving it in the `orderbook`. A new auction replaces the current one and uses the value of the `auctions_id_seq` sequence and increase it to ensure that auction ids are unique and monotonically increasing.

 Column | Type   | Nullable | Details
--------|--------|----------|--------
 id     | bigint | not null | other tables refer to this as auction\_id
 json   | jsonb  | not null | serialized version of the auction. Technically the format is unspecified. The only requirement is that whatever format the `autopilot` stores can be parsed by the `orderbook`.

Indexes:
- PRIMARY KEY: btree(`id`)

### ethflow\_orders

EthFlow orders get created with the very generic [`ICoWSwapOnchainOrders`](https://github.com/cowprotocol/ethflowcontract/blob/1d5d54a4ba890c5c0d3b26429ee32aa8e69f2f0d/src/interfaces/ICoWSwapOnchainOrders.sol#L6-L50) smart contract interface. However this interface doesn't return all the information that is required for EthFlow orders. This extra data is stored here whereas the generic data is stored in [onchain\_placed\_orders](#onchain\_placed\_orders).

 Column    | Type   | Nullable | Details
-----------|--------|----------|--------
 uid       | bytea  | not null | other tables refer to this as order\_uid
 valid\_to | bigint | not null | unix timestamp in seconds when the order expires (the native timestamp format in the EVM)

Indexes:
- PRIMARY KEY: btree(`uid`)

### ethflow\_refunds

For orders buying some token with native ETH users temporarily transfer ownership of their ETH to the ethflow contract. When their order expires the `refunder` service automatically returns the ETH to the user. The table stores data about the transactions that refunded expired orders.

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 order\_uid    | bytea  | not null | order that got refunded
 block\_number | bigint | not null | in which block the order got refunded
 tx\_hash      | bytea  | not null | hash of the transaction that refunded the order

Indexes:
- PRIMARY KEY: btree(`order_uid`)

### flyway\_schema\_history

We use flyway to do migrations of our database schema. This table contains metadata for flyway to know which and when migrations have been applied. Since this table only contains data managed by flyway and we didn't encounter any need to take a closer look at it we'll just refer to the [flyway docs](https://flywaydb.org/documentation/).

### interactions

The settlement contract allows associating user provided interactions to be executed before and after an order. This table stores these interactions and associates them with the respective orders.

 Column     | Type                   | Nullable | Details
------------|------------------------|----------|--------
 order\_uid | bytea                  | not null | order that this interaction belongs to
 index      | integer                | not null | index indicating in which interactions should be executed in case the same order has multiple interactions (ascending order)
 target     | bytea                  | not null | address of the smart contract this interaction should call
 value      | numeric                | not null | amount of ETH this interaction should send to the smart contract
 data       | bytea                  | not null | call data that contains the function selector and the bytes passed to it
 execution  | [enum](#executiontime) | not null | in which phase the interaction should be executed

Indexes:
- PRIMARY KEY: btree(`order_uid`)


### invalidations

Stores data of [`OrderInvalidated`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L70-L71) events emitted by [`invalidateOrder()`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L244-L255) of the settlement contract.

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 block\_number | bigint | not null | block in which the event was emitted
 log\_index    | bigint | not null | index in which the log was emitted
 order\_uid    | byteai | not null | order that got invalidated

Indexes:
- PRIMARY KEY: btree(`block_number, log_index`)
- invalidations\_order\_uid: btree(`order_uid`, `block_number`, `log_index`)

### onchain\_order\_invalidations

Stores data of [`OrderInvalidation`](https://github.com/cowprotocol/ethflowcontract/blob/main/src/interfaces/ICoWSwapOnchainOrders.sol#L46-L49) events emitted by the `ICoWSwapOnchainOrders` interface.

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 block\_number | bigint | not null | block in which the event was emitted
 log\_index    | bigint | not null | index in which the log was emitted
 uid           | byteai | not null | order that got invalidated

Indexes:
- PRIMARY KEY: btree(`uid`)
- invalidation\_event\_index: btree(`block_number, log_index`)

### onchain\_placed\_orders

Stores data of [`OrderPlacement`](https://github.com/cowprotocol/ethflowcontract/blob/main/src/interfaces/ICoWSwapOnchainOrders.sol#L23-L44) events emitted by the `ICoWSwapOnchainOrders` interface plus some metadata.

 Column           | Type                                | Nullable | Details
------------------|-------------------------------------|----------|--------
 uid              | bytea                               | not null | order that got created also known as order\_uid
 sender           | bytea                               | not null | user that created the order with the smart contract
 is\_reorged      | boolean                             | not null | if the backend detects that a block creating an order got reorged it gets invalidated with this flag
 block\_number    | bigint                              | not null | block in which the order was created
 log\_index       | bigint                              | not null | index in which the `OrderPlacement` event was emitted
 placement\_error | [enum](#onchainorderplacementerror) | nullable | what error happened when placing the order

Indexes:
- PRIMARY KEY: btree(`uid`)
- event\_index: btree(`block_number`, `index`)
- order\_sender: hash(sender)

### order\_events

Stores timestamped events throughout an order's life cycle. This information is used to get detailed metrics on a per order basis.

 Column           | Type                     | Nullable | Details
------------------|--------------------------|----------|--------
 order\_uid       | bytea                    | not null | order this event belongs to
 timestamp        | timestamptz              | not null | when the event was registered
 label            | [enum](#ordereventlabel) | not null | which event happened exactly

Indexes:
- order\_events\_by\_uid: btree(`order_uid`, `timestamp`)

### order\_execution

Contains metainformation for trades, required for reward computations that cannot be recovered from the blockchain and are not stored in a persistent manner somewhere else.

 Column       | Type    | Nullable | Details
--------------|---------|----------|--------
 order\_uid   | bytea   | not null | which order this trade execution is related to
 auction\_id  | bigint  | not null | in which auction this trade was initiated
 reward       | double  | not null | revert adjusted solver rewards, deprecated in favor of [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f)
 surplus\_fee | numeric | nullable | dynamic fee computed by the protocol that should get taken from the surplus of a trade, this value only applies and is set for fill-or-kill limit orders.
 block\_number| bigint  | not null | block in which the order was executed

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

Quotes that an order was created with. These quotes get stored persistently and can be used to evaluate how accurate the quoted fee predicted the execution cost that actually happened on-chain.

 Colmun             | Type    | Nullable | Details
--------------------|---------|----------|--------
 order\_uid         | bytea   | not null | order that this quote belongs to
 gas\_amount        | double  | not null | estimated gas used by the quote used to create this order with
 gas\_price         | double  | not null | gas price at the time of order creation
 sell\_token\_price | double  | not null | ether-denominated price of sell\_token at the time of quoting. The ether value of `x` sell\_tokens is `x * sell_token_price`.
 sell\_amount       | numeric | not null | sell\_amount of the quote used to create the order with
 buy\_amount        | numeric | not null | buy\_amount of the quote used to create the order with
 solver             | bytea   | not null | public address of the solver that provided this quote

Indexes:
- PRIMARY KEY: btree(`order_uid`)

### orders

Contains all relevant signed data of an order and metadata that is important for correctly executing the order with the [GPv2Settlement](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol) smart contract.

Column                    | Type                         | Nullable | Details
--------------------------|------------------------------|----------|--------
 uid                      | bytea                        | not null | 56 bytes identifier composed of a 32 bytes `hash` over the order data signed by the user, 20 bytes containing the `owner` and 4 bytes containing `valid_to`.
 owner                    | bytea                        | not null | address who created this order and where the sell\_token will be taken from, note that for ethflow orders this is the [CoWSwapEthFlow](https://github.com/cowprotocol/ethflowcontract/blob/main/src/CoWSwapEthFlow.sol) smart contract and not the user that actually initiated the trade
 creation\_timestamp      | timestamptz                  | not null | when the order was created
 sell\_token              | bytea                        | not null | address of the token that will be sold
 buy\_token               | bytea                        | not null | address of the token that will be bought
 sell\_amount             | numeric                      | not null | amount in sell\_token that should be sold at most
 buy\_amount              | numeric                      | not null | amount of buy\_token that should be bought at least
 valid\_to                | timestamptz                  | not null | point in time when the order can no longer be settled
 fee\_amount              | numeric                      | not null | amount in sell\_token the owner agreed upfront as a fee to be taken for the trade
 kind                     | [enum](#orderkind)           | not null | trade semantics of the order
 partially\_fillable      | bool                         | not null | determines if the order can be executed in multiple smaller trades or if everything has to be executed at once (fill-or-kill)
 signature                | bytea                        | not null | signature provided by the owner stored as raw bytes. What these bytes mean is determined by signing\_scheme
 cancellation\_timestamp  | timestamptz                  | nullable | when the order was cancelled. If the timestamp is null it means the order has not been cancelled yet
 receiver                 | bytea                        | nullable | address that should receive the buy\_tokens. If this is null the owner will receive the buy tokens
 app\_data                | bytea                        | not null | arbitrary data associated with this order but per [design](https://docs.cow.fi/cow-sdk/order-meta-data-appdata) this is an IPFS hash which may contain additional meta data for this order signed by the user
 signing\_scheme          | [enum](#signingscheme)       | not null | what kind of signature was used to proof that the `owner` actually created the order
 settlement\_contract     | bytea                        | not null | address of the contract that should be used to settle this order
 sell\_token\_balance     | [enum](#selltokensource)     | not null | defines how sell\_tokens need to be transferred into the settlement contract
 buy\_token\_balance      | [enum](#buytokendestination) | not null | defined how buy\_tokens need to be transferred back to the user
 full\_fee\_amount        | numeric                      | not null | estimated execution cost in sell\_token of this order
 class                    | [enum](#orderclass)          | not null | determines which special trade semantics will apply to the execution of this order


Indexes:
- PRIMARY KEY: btree(`uid`)

### fee_policies

Contains all relevant data of fee policies applied to orders during auctions.

Column                               | Type                         | Nullable | Details
-------------------------------------|------------------------------|----------|--------
 auction_id                          | bigint                       | not null | unique identifier for the auction
 order_uid                           | bytea                        | not null | 56 bytes identifier linking to the order in the `orders` table
 application_order                   | serial                       | not null | the order in which the fee policies are inserted and applied
 kind                                | [PolicyKind](#policykind)    | not null | type of the fee policy, defined in the PolicyKind enum
 surplus_factor                      | double precision             |          | percentage of the surplus for fee calculation; value is between 0 and 1
 surplus_max_volume_factor           | double precision             |          | cap for the fee as a percentage of the order volume; value is between 0 and 1
 volume_factor                       | double precision             |          | fee percentage of the order volume; value is between 0 and 1
 price_improvement_factor            | double precision             |          | percentage of the price improvement over the best quote received during order creation; value is between 0 and 1
 price_improvement_max_volume_factor | double precision             |          | cap for the fee as a percentage of the order volume; value is between 0 and 1

Indexes:
- PRIMARY KEY: composite key(`auction_id`, `order_uid`, `application_order`)

#### Enums

- #### PolicyKind
    Enum for the `kind` column in `fee_policies` table.

    Values:
    - `surplus`: The fee is based on the surplus achieved in the trade.
    - `priceimprovement`: The fee is based on a better executed price than the top quote.
    - `volume`: The fee is based on the volume of the order.

### presignature\_events

Stores data of [`PreSignature`](https://github.com/cowprotocol/contracts/blob/5e5c28877c1690415548de7bc4b5502f87e7f222/src/contracts/mixins/GPv2Signing.sol#L59-L61) events. This is a mechanism where users can supply a signature for an order\_uid even before creating the original order in the backend. These events can give or revoke a signature.


 Column        | Type    | Nullable | Details
---------------|---------|----------|--------
 block\_number | bigint  | not null | block in which the event was emitted
 log\_index    | bigint  | not null | index in which the event was emitted
 owner         | bytea   | not null | owner of the order
 order\_uid    | bytea   | not null | order for which the signature was given or revoked
 signed        | boolean | not null | specifies if an a signature was given or revoked

Indexes:
- PRIMARY KEY: btreebtree(`block_number`, `log_index`)
- most\_recent\_with\_orderuid: btree (`order_uid`, `block_number` DESC, `log_index` DESC)
- presignature\_owner: hash(`owner`)

### quotes (and quotes\_id\_seq counter)

Stores quotes in order to determine whether it makes sense to allow a user to create an order with a given `fee_amount`. Quotes are short lived and get deleted when they expire. `id`s are unique and increase monotonically.

 Column                | Type               | Nullable | Details
-----------------------|--------------------|----------|--------
 sell\_token           | bytea              | not null | address of the token that should be sold
 sell\_amount          | numeric            | not null | amount that should be sold at most
 buy\_token            | bytea              | not null | address of token that should be bought
 buy\_amount           | numeric            | not null | amount that should be bought at least
 expiration\_timestamp | timestamptz        | not null | when the quote should no longer be considered valid. Invalid quotes will get deleted shortly
 order\_kind           | [enum](#orderkind) | not null | trade semantics for the quoted order
 gas\_amount           | double             | not null | estimation of gas used to execute the order according to the quote
 gas\_price            | double             | not null | gas price at the time of quoting
 sell\_token\_price    | double             | not null | price of sell\_token in ETH. Since fees get taken in the sell token the actual fee will be computed with `sell_token_price * gas_amount * gas_used`.
 id                    | bigint             | not null | unique identifier of this quote
 quote\_kind           | [enum](#quotekind) | not null | quotekind for which this quote is considered valid
 solver                | bytea              | not null | public address of the solver that provided this quote

Indexes:
- PRIMARY KEY: btree(`id`)
- quotes\_token\_expiration: btree (`sell_token`, `buy_token`, `expiration_timestamp` DESC)


### settlement\_observations

During the solver competition solvers promise a solution of a certain quality. If the settlement that eventually gets executed on-chain is worse than what was promised solvers can get slashed. This table stores the quality of the solution that was actually observed on-chain. (see [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f))

 Column                | Type    | Nullable | Details
-----------------------|---------|----------|--------
 block\_number         | bigint  | not null | block in which the settlement happened
 log\_index            | bigint  | not null | index of the [`Settlement`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L67-L68) event
 gas\_used             | numeric | not null | amount of gas the settlement consumed
 effective\_gas\_price | numeric | not null | effective gas price (basically the [EIP-1559](https://eips.ethereum.org/EIPS/eip-1559) gas price reduced to a single value)
 surplus               | numeric | not null | amount of tokens users received more than their limit price converted to ETH
 fee                   | numeric | not null | total amount of fees collected in the auction

Indexes:
- PRIMARY KEY: btree(`block_number`, `log_index`)
- settlements\_auction\_id: btree(`auction_id`)

### settlement\_scores

Stores the best and second best solution quality (score) of every auction promised by solvers for [CIP-20](https://snapshot.org/#/cow.eth/proposal/0x2d3f9bd1ea72dca84b03e97dda3efc1f4a42a772c54bd2037e8b62e7d09a491f) reward computation.

 Column           | Type     | Nullable | Details
------------------|----------|----------|--------
 auction\_id      | bigint   | not null | id of the auction the scores belong to
 winner           | bytea    | not null | public address of the winning solver
 winning\_score   | numeric  | not null | highest submitted score (submitted by `winner`). This is the quality the auction observed on-chain should achieve to not result in slashing of the solver.
 reference\_score | numeric  | not null | score of the runner up solver. If only 1 solver submitted a valid solution this value is 0.
 block\_deadline  | bigint   | not null | block at which the solver should have executed the solution at the latest before getting slashed for executing too slowly
 simulated_block  | bigint   | not null | block at which the simulation of the competing solutions is done

Indexes:
- PRIMARY KEY: btree(`auction_id`)

### settlement\_call\_data

Stores the final calldata and uninternalized calldata of the winning solution for each auction

 Column                       | Type     | Nullable | Details
------------------------------|----------|----------|--------
 auction\_id                  | bigint   | not null | id of the auction the winning transaction calldata belongs to
 call_data                    | bytea    | not null | final calldata as it appears on the blockchain
 uninternalized\_call\_data   | numeric  | not null | uninternalized calldata, different from final calldata if solution contains interactions that can be internalized against gpv2 settlement contract internal buffers.

Indexes:
- PRIMARY KEY: btree(`auction_id`)

### settlements

Stores data and metadata of [`Settlement`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L67-L68) events emitted from the settlement contract.

 Column        | Type   | Nullable | Details
---------------|--------|----------|--------
 block\_number | bigint | not null | block in which the settlement happened
 log\_index    | bigint | not null | index in which the event was emitted
 solver        | bytea  | not null | public address of the executing solver
 tx\_hash      | bytea  | not null | transaction hash in which the settlement got executed
 tx\_from      | bytea  | not null | address that submitted the transaction (same as `solver`)
 tx\_nonce     | bigint | not null | nonce that was used to submit the transaction

Indexes:
- PRIMARY KEY: btree(`block_number`,`log_index`)
- settlements\_tx\_from\_tx\_nonce: btree(`tx_from`, `tx_nonce`)
- settlements\_tx\_hash: hash(`tx_hash`)

### solver\_competitions

Stores an overview of the solver competition. It contains orders in the auction along with prices for every relevant token as well as all valid solutions submitted by solvers together with their quality.

 Column | Type   | Nullable | Details
--------|--------|----------|--------
 id     | bigint | not null | id of the auction that the solver competition belongs to
 json   | jsonb  | nullable | overview of the solver competition with unspecified format

Indexes:
- PRIMARY KEY: btree(`id`)

### trades

This table contains data of [`Trade`](https://github.com/cowprotocol/contracts/blob/main/src/contracts/GPv2Settlement.sol#L49-L58) events issued by the settlement contract after a successful settlement.

 Column        | Type    | Nullable | Details
---------------|---------|----------|--------
 block\_number | bigint  | not null | block in which the event happened
 log\_index    | bigint  | not null | index in which the event was emitted
 order\_uid    | bytea   | not null | trade filled this order partially or completely
 sell\_amount  | numeric | not null | amount of sell\_token that got taken from the order owner
 buy\_amount   | numeric | not null | amount of buy\_token received by the order owner
 fee\_amount   | numeric | not null | fee amount in sell\_token that got taken in this trade. Note that this amount refers to all or a portion of the static fee\_amount the user signed during the order creation.

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
 quote\_not\_found               | order was created with an expired quote
 invalid\_quote                  | the associated quote does not apply to the order
 pre\_validation\_error          | basic pre order creation check failed (e.g. no 0 amounts)
 disabled\_order\_class          | unused
 valid\_to\_too\_far\_in\_future | unused
 invalid\_order\_data            | unused
 insufficient\_fee               | the proposed fee is less than quoted fee
 non\_zero\_fee                  | the proposed fee is not zero
 other                           | some unexpected error happened

#### ordereventlabel

 Value      | Meaning
------------|--------
 created    | order was added to the orderbook
 ready      | order was included in an auction and sent to solvers
 filtered   | order was filtered from the auction and not sent to solvers
 invalid    | order can not be settled on-chain (e.g. user is missing funds, PreSign or EIP-1271 signature is invalid, etc.)
 executing  | order was included in the winning solution and is in the process of being submitted on-chain
 considered | order was in a valid solution
 traded     | order was traded on-chain
 cancelled  | user cancelled the order

#### orderkind

 Value | Meaning
-------|--------
 sell  | the order sells the entire sell\_amount for at least the user signed buy\_amount
 buy   | the order buys the entire buy\_amount for at most the user signed sell\_amount

#### signingscheme

 Value   | Meaning
---------|--------
 presign | User broadcasts a transaction onchain containing a signature of the order hash. Because this onchain transaction is also signed, it proves that the user indeed signed the order.
 ethsign | Standardized way to sign arbitraty bytes ([EIP-191](https://eips.ethereum.org/EIPS/eip-191))
 eip712  | Standardized way to hash and sign structured data. ([eip712](https://eips.ethereum.org/EIPS/eip-712))
 eip1271 | Owner of the order is a smart contract that implements [EIP-1271](https://eips.ethereum.org/EIPS/eip-1271). To verify that the order is allowed to execute we call the owner's `isValidSignature(order_hash, signature)` function and let it decide. Used to implement [smart orders](https://docs.cow.fi/tutorials/how-to-place-erc-1271-smart-contract-orders/smart-orders).

#### quotekind

We support different expiration times for orders with different signing schemes. This is because offline signed messages can immediately be validated but presign or eip-1271 signatures need to interact with the blockchain which may take time. This could be achieved by simply setting the appropriate `expiration_timestamp` in the quote. But we also want to prevent users from creating for example quick `eip712` orders with long living quotes intended for `eip1271` orders which might be way off by then so quotes also get tagged with this `quotekind`.

 Value               | Meaning
---------------------|--------
 standard            | Quote for `eip712` or `ethsign` orders.
 eip1271onchainorder | Quote that accounts for gas used to verify signature with on-chain `isValidSignature()` call (see [signingscheme::eip1271](#signingscheme))
 presignonchainorder | Quote for `presign` orders.

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
 liquidity | These orders must be traded at their limit price and may not receive any surplus. Violating this is a slashable offence.
 limit     | Long lived order that may receive surplus. Users sign a static fee of 0 upfront and either the backend or the solvers compute a dynamic fee that gets taken from the surplus (while still respecting the user's limit price!).
