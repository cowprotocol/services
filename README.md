![pull request](https://github.com/gnosis/gp-v2-services/workflows/pull%20request/badge.svg) ![deploy](https://github.com/gnosis/gp-v2-services/workflows/deploy/badge.svg)

# GPv2 Services

This repository contains backend code for [Gnosis Protocol V2](https://docs.gnosis.io/protocol/) written in Rust.

## Order Book

The `orderbook` crate provides the http api through which users (usually through a frontend web application) interact with the order book.
Users can add signed orders to the order book and query the state of their orders.
They can also use the API to estimate fee amounts and limit prices before placing their order.

Solvers also interact with the order book by querying a list of open orders that they can attempt to settle.

The api is documented with [openapi](https://protocol-rinkeby.dev.gnosisdev.com/api/).
A simple example script that uses the API to place random orders can be found in [this repo](https://github.com/gnosis/gp-v2-trading-bot)

The order book service itself uses PostgreSQL as a backend to persist orders.
In addition to connecting the http api to the database it also checks order validity based on the block time, trade events, erc20 funding and approval so that solvers can query only valid orders.

## Solver

The `solver` crate is responsible for submitting on chain settlements based on the orders it gets from the order book and other liquidity sources like Balancer or Uniswap pools.

It implements a few settlement strategies directly in Rust:

- Naive Solver: Can match to overlapping opposing orders (e.g. DAI for WETH & WETH for DAI) with one another settling the excess with Uniswap
- Uniswap Baseline: Same path finding as used by the Uniswap frontend (settling orders individually instead of batching them together)

It can can also interact with a more advanced, Gnosis internal, closed source solver which tries to settle all orders using the combinatorial optimization formulations described in [Multi-Token Batch Auctions with Uniform Clearing Price](https://github.com/gnosis/dex-research/blob/master/BatchAuctionOptimization/batchauctions.pdf)

## Other Crates

Several pieces of functionality are shared between the order book and the solver. They live in other crates in the cargo workspace.

- `contract` provides _[ethcontract-rs](https://github.com/gnosis/ethcontract-rs)_ based smart contract bindings
- `model` provides the serialization model for orders in the order book api
- `shared` provides other shared functionality between the solver and order book

## Testing

The CI runs unit tests, e2e tests, `clippy` and `cargo fmt`

### Unit Tests:

`cargo test`

### Integration Tests:

`cargo test --jobs 1 -- --ignored --test-threads 1 --skip http_solver`

**Note:** Requires postgres database running (see below).

### E2E Tests

`cargo test -p e2e`.

**Note:** Requires postgres database and local test network with smart contracts deployed (see below).

### Clippy

`cargo clippy --all-features --all-targets -- -D warnings`

## Development Setup

### Postgres

The tests that require postgres connect to the default database of a locally running postgres instance on the default port. There are several ways to set up postgres:

- Docker

```sh
docker run -d -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_USER=`whoami` -p 5432:5432 docker.io/postgres
```

- Host System Service

```sh
sudo systemctl start postgresql.service
sudo -u postgres createuser $USER
sudo -u postgres createdb $USER
```

- Manual setup in local folder

```sh
mkdir postgres && cd postgres
initdb data # Arbitrary directory that stores the database
# In data/postgresql.conf set unix_socket_directories to the absolute path to an arbitrary existing
# and writable directory that postgres creates a temporary file in.
# Run postgres
postgres -D data
# In another terminal, only for first time setup
createdb -h localhost $USER
```

<br>

At this point the database should be running and reachable. You can test connecting to it with

```sh
psql postgresql://localhost/
```

### DB Migration/Initialization

Finally, we need to apply the schema (set up in the `database` folder). Again, this can be done via docker or locally:

- Docker

```sh
docker build --tag gp-v2-migrations -f docker/Dockerfile.migration .
# If you are running postgres in locally, your URL is `localhost` instead of `host.docker.internal`
docker run -ti -e FLYWAY_URL="jdbc:postgresql://host.docker.internal/?user="$USER"&password=" -v $PWD/database/sql:/flyway/sql gp-v2-migrations migrate
```

If you're combining a local postgres installation with docker flyway you have to add to the above `--network host` and change `host.docker.internal` to `localhost`.

- Local [flyway installation](https://flywaydb.org/documentation/usage/commandline/#download-and-installation)

```sh
flyway -user=$USER -password="" -locations="filesystem:database/sql/" -url=jdbc:postgresql:/// migrate
```

### Local Test Network

With a testnet (e.g. [Ganache](https://www.trufflesuite.com/ganache)) running on `localhost:8545` deploy the contracts via:

```sh
cd contracts; cargo run --bin deploy --features bin; cd -
```

## Running the services

- `cargo run --bin orderbook -- --help`
- `cargo run --bin solver -- --help`

To test the system end to end checkout the [GPv2 UI](https://github.com/gnosis/gp-swap-ui) and point it to your local instance.
