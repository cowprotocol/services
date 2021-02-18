![pull request](https://github.com/gnosis/gp-v2-services/workflows/pull%20request/badge.svg) ![deploy](https://github.com/gnosis/gp-v2-services/workflows/deploy/badge.svg)

# gp-v2-services

This repository contains backend code for [Gnosis Protocol V2](TODO high level gpv2 link) written in Rust.

## Order Book

The `orderbook` crate provides the http api through which users (usually through a frontend web application) interact with the order book.
Users can add signed orders to the order book and query the state of their orders.

Solvers also interact with the order book by querying a list of open orders that they can attempt to settle.

The api is documented with [openapi](https://protocol-rinkeby.dev.gnosisdev.com/api/).

The order book service itself uses PostgreSQL as a backend to persist orders.
In addition to connecting the http api to the database it also checks order validity based on the block time, trade events, erc20 funding and approval so that solvers can query only valid orders.

## Solver

The `solver` crate is responsible for submitting on chain settlements based on the orders it gets from the order book and other liquidity sources like uniswap pools.

It implements a naive solver directly in Rust and can also interact with a more advanced, Gnosis internal, closed source solver.

## Other Crates

Several pieces of functionality are shared between the order book and the solver. They live in other crates in the cargo workspace.

- `contract` provides ethcontract based smart contract bindings
- `model` provides the serialization model for orders in the order book api
- `shared` provides other shared functionality between solver and order book

## Testing

Run unit tests: `cargo test`.

Some unit tests and the e2e tests require a local postgres setup. For some ways on how to start postgres see the next section. e2e tests additionally require ganache to be running.

Run postgres unit tests: `cargo test --jobs 1 postgres -- --ignored --test-threads 1`

Run e2e tests: `cargo test -p e2e -- --test-threads 1`.

### Postgres

The tests that require postgres connect to the default database of locally running postgres instance on the default port. There are several ways to set up postgres:

```sh
# Docker
docker run -d -e POSTGRES_HOST_AUTH_METHOD=trust -e POSTGRES_USER=`whoami` -p 5432:5432 postgres

# Service
sudo systemctl start postgresql.service
sudo -u postgres createuser $USER
sudo -u postgres createdb $USER

# Manual setup in local folder
mkdir postgres && cd postgres
initdb data # Arbitrary directory that stores the database
# In data/postgresql.conf set unix_socket_directories to the absolute path to an arbitrary existing
# and writable directory that postgres creates a temporary file in.
# Run postgres
postgres -D data
# In another terminal, only for first time setup
createdb -h localhost $USER

# Finally for all methods to test that the server is reachable and to set the schema for the tests.
docker build --tag gp-v2-migrations -f docker/Dockerfile.migration .
# If you are running postgres in locally, your URL is `localhost` instead of `host.docker.internal`
docker run -ti -e FLYWAY_URL="jdbc:postgresql://host.docker.internal/?user="$USER"&password=" -v $PWD/database/sql:/flyway/sql gp-v2-migrations migrate
```

## Running

- `cargo run --bin orderbook -- --help`
- `cargo run --bin solver -- --help`
