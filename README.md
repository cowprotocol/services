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

* `contract` provides ethcontract based smart contract bindings
* `model` provides the serialization model for orders in the order book api
* `shared` provides other shared functionality between solver and order book

## Testing

Run unit tests with `cargo test`.
Some (by default ignored) tests require a locally running Postgres instance as seen on [CI](.github/workflows/pull-request.yaml).
More extensive end to end tests can be run with `cargo test -p e2e`.
These require a locally running instance of ganache.

A more extensive e2e test using ganache

## Running

* `cargo run --bin orderbook -- --help`
* `cargo run --bin solver -- --help`
