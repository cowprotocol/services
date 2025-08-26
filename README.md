![pull request](https://github.com/cowprotocol/services/workflows/pull%20request/badge.svg) ![deploy](https://github.com/cowprotocol/services/workflows/deploy/badge.svg)

# Cow Protocol Services

This repository contains backend code for [Cow Protocol Services](https://docs.cow.fi/) written in Rust.

## Order Book

The `orderbook` crate provides the http api through which users (usually through a frontend web application) interact with the order book.
Users can add signed orders to the order book and query the state of their orders.
They can also use the API to estimate fee amounts and limit prices before placing their order.

Solvers also interact with the order book by querying a list of open orders that they can attempt to settle.

The api is documented with [openapi](https://api.cow.fi/docs/).
A simple example script that uses the API to place random orders can be found in [this repo](https://github.com/cowprotocol/trading-bot)

The order book service itself uses PostgreSQL as a backend to persist orders.
In addition to connecting the http api to the database it also checks order validity based on the block time, trade events, erc20 funding and approval so that solvers can query only valid orders.

Multiple concurrent `orderbook`s can run at the same time, allowing the user-facing API to scale horizontally with increased traffic.

## Autopilot

The `autopilot` crate is responsible for driving the protocol forward.
Concretely, it is responsible for "cutting" new auctions (i.e. determining auction boundaries and which orders are to be included, as well as various parameters important for settlement objective value computation).

The `autopilot` connects to the same PostgreSQL database as the `orderbook` and uses it to query orders as well as storing the most recent auction and settlement competition.

## Other Crates

There are additional crates that live in the cargo workspace.

- `alerter` provides a custom alerter binary that looks at the current orderbook and counts metrics for orders that should be solved but aren't
- `contracts` provides _[ethcontract-rs](https://github.com/gnosis/ethcontract-rs)_ based smart contract bindings
- `database` provides the shared database and storage layer logic shared between the `autopilot` and `orderbook`
- `driver` an in-development binary that intends to replace the `solver`; it has a slightly different design that allows co-location with external solvers
- `e2e` end-to-end tests
- `ethrpc` ethrpc client with a few extensions
- `model` provides the serialization model for orders in the order book api
- `number` extensions to number types, such as numerical conversions between 256-bit integers, nonzero types and de/serialization implementations
- `observe` initialization and helper functions for logging and metrics
- `shared` provides other shared functionality between the solver and order book
- `testlib` shared helpers for writing unit and end-to-end tests

## Running the Services Locally

To run the services locally you should use the [`playground`](./playground/README.md).
You can launch it with the following command:

```
docker compose up -f playground/docker-compose.fork.yml up -d
```

You can read more about the services available and their respective ports in the [playground's README](./playground/README.md).

## Testing

The CI (check [`.github/workflows/pull-request.yaml`](.github/workflows/pull-request.yaml)) runs
[doc-tests](https://github.com/cowprotocol/services/tree/main/.github/workflows/pull-request.yaml#L71),
[unit tests](https://github.com/cowprotocol/services/tree/main/.github/workflows/pull-request.yaml#L88-L89),
[DB tests](https://github.com/cowprotocol/services/tree/main/.github/workflows/pull-request.yaml#L117),
[E2E tests with a local node](https://github.com/cowprotocol/services/tree/main/.github/workflows/pull-request.yaml#L147),
[E2E tests with a forked node](https://github.com/cowprotocol/services/tree/main/.github/workflows/pull-request.yaml#L187) and
[driver tests](https://github.com/cowprotocol/services/tree/main/.github/workflows/pull-request.yaml#L206-L209) (and more).
The CI system uses [cargo-nextest](https://nexte.st/) and therefor all tests are getting verified by it.
`cargo-nextest` and `cargo test` handle global state slightly differently which can cause some tests to fail with `cargo test`.
That's why it's recommended to run tests with `cargo nextest run`.

### DB & E2E Tests

Database and E2E tests require some infrastructure, following is their setup process.

### Postgres

The tests that require postgres connect to the default database of a locally running postgres instance on the default port.
To achieve this, open a new shell and run the command below:
Note: The migrations will be applied as well.

```sh
docker-compose up
```

### Local Test Network

In order to run the `e2e` tests you have to have an EVM compatible testnet running locally.
We make use of [anvil](https://github.com/foundry-rs/foundry) from the Foundry project to spin up a local testnet.

`anvil` supports all the RPC methods we need to run the services and tests.

1. Install [foundryup](https://book.getfoundry.sh/getting-started/installation).
2. Install foundry with `foundryup`.
3. Run `anvil` with the following configuration:

```bash
ANVIL_IP_ADDR=0.0.0.0 anvil \
  --gas-price 1 \
  --gas-limit 10000000 \
  --base-fee 0 \
  --balance 1000000 \
  --chain-id 1 \
  --timestamp 1577836800
```

## Profiling

All binaries are compiled with support for [tokio-console](https://github.com/tokio-rs/console) by default to allow you to look inside the tokio runtime.
However, this feature is not enabled at runtime by default because it comes with a pretty significant memory overhead. To enable it you just have to set the environment variable `TOKIO_CONSOLE=true` and run the binary you want to instrument.

You can install and run `tokio-console` with:
```bash
cargo install --locked tokio-console
tokio-console
```

## Changing Log Filters

It's possible to change the tracing log filter while the process is running. This can be useful to debug an error that requires more verbose logs but which might no longer appear after restarting the system.

Each process opens a UNIX socket at `/tmp/log_filter_override_<program_name>_<pid>.sock`. To change the log filter connect to it with `nc -U <path>` and enter a new log filter.
You can also reset the log filter to the filter the program was initially started with by entering `reset`.

See [here](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives) for documentation on the supported log filter format.

