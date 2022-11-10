# Architecture

If you haven't already, please see the [guidelines document](./guidelines.md) first.

## Crates and modules

The codebase would be simpler without crates. I'd like to make the following arguments for
modules and against crates:

- Dead code analysis
- Having to explicitly encode dependencies between crates
- Workspace management is more complex than managing a single binary

Instead I propose we use command line arguments to run specific subsystems:

```
$ binary_name command_name --flags
```

And collapse all of our crates into a single crate with modules.

## Proposed core logic structure

Top level module: `cowswap`. Submodules: `order`, `quote`, `solution`.

### `order`

Hides details related to order creation, validation, querying, etc. The main types
exposed by this module would be `Order` and `OrderBook`.

### `quote`

Hides details related to quote creation, including fee and price estimation, fee subsidies,
etc. The main types exposed by this module are `Quote` and `Quoter`.

External traits: `TransactionSimulator`, maybe something like `TransactionExplorer` for
looking up onchain transactions, maybe `FeeEstimator` for estimating onchain fees.

### `solution`

Hides details related to finding a solution and publishing it to the blockchain. This could
implement two submodules: `solver` and `settlement`, but don't export these, instead reexport
types in these modules.

`solver` would define the logic related to our and external solvers. This module would implement
what we refer to as "driver" today, though I'm not sure if we want to keep that name. The main
type exported from this module is `Solver`. Personally I'm not a fan of the current "recursive"
design that we have with the solver trait - if we do decide to have a trait for solvers, this
trait should not be exposed, and instead we should only expose the concrete type that represents
the aggregate solver.

Not sure what the external traits would be for `solver`.

`settlement` would define the logic related to onchain settlement. There is a question of where the
logic related to signature validation should reside. Since the signature needs to be verified during
order creation, and signatures are used to sign _orders_ to begin with, `order` is the right
place for this. The main types exposed by this module are `Settlement` and `Settler`. It would also
contain some logic related to listening to onchain transactions in the background and reacting to those.

External traits for `settlement` might be `SettlementContract` to make an onchain call into the contract,
probably others like `TransactionListener` for listening to onchain transactions.

### Dependencies

`order` depends on `quote`. `solution` depends on `order`.

## Other structure

I think I'd have the following CLI subcommands:

- `orderbook`, which starts an orderbook API server
- `solver`, which runs the driver
- `background`, which runs the background singleton (I don't want to use the name `autopilot` since it's
not very intuitive, `autopilot` sounds like it automates in a simple way something that is
normally done manually most of the time. For example [LND](https://github.com/lightningnetwork/lnd) has an autopilot mode for opening
channels, and while it works, it's not nearly as good as opening channels manually.)

However, these are infrastructural details and the core logic behind these commands is still implemented
in the core logic modules outlined above.
