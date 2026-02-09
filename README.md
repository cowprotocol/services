[![pull request](https://github.com/cowprotocol/services/workflows/pull%20request/badge.svg)](https://github.com/cowprotocol/services/actions/workflows/pull-request.yaml) [![deploy](https://github.com/cowprotocol/services/workflows/deploy/badge.svg)](https://github.com/cowprotocol/services/actions/workflows/deploy.yaml)

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

- `contracts` provides Alloy-based smart contract bindings
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
docker compose -f playground/docker-compose.fork.yml up --build
```

Optionally you can limit the services run by the playground by specifying the desired service's names (ex. `driver autopilot`).

Once stabilized, the playground will watch your local directory for changes and automatically recompile and restart the services as needed.

You can read more about the services available and their respective ports in the [playground's README](./playground/README.md).

> Binaries like `autopilot`, `orderbook`, `driver` and `solvers` have `-h` and `--help` for, respectively, short and long descriptions over available commands and options.
> Furthermore, there's OpenAPI pages for the [`orderbook`](https://docs.cow.fi/cow-protocol/reference/apis/orderbook),
> [`driver`](https://docs.cow.fi/cow-protocol/reference/apis/driver) and [`solver`](https://docs.cow.fi/cow-protocol/reference/apis/solver) APIs,
> you can find more information about the services and the CoW Protocol at <docs.cow.fi>.


## Testing

The CI (check [`.github/workflows/pull-request.yaml`](.github/workflows/pull-request.yaml)) runs
- doc tests: `just test-doc`
- unit tests: `just test-unit`
- DB tests: `just test-db`
- E2E tests with a local node: `just test-e2e-local`
- E2E tests with a forked node: `just test-e2e-forked`
- driver tests: `just test-driver`

The CI system uses [cargo-nextest](https://nexte.st/) and therefore all tests are getting verified by it.
`cargo-nextest` and `cargo test` handle global state slightly differently which can cause some tests to fail with `cargo test`.
That's why it's recommended to run tests with the provided `just` commands.

### Flaky Tests

In case a test is flaky and only fails **sometimes** in CI you can use the [`run-flaky-test`](.github/workflows/pull-request.yaml) github action to test your fix with the CI to get confidence that the fix that works locally also works in CI.

### Postgres

The tests that require postgres connect to the default database of a locally running postgres instance on the default port.
To achieve this, open a new shell and run the command below:
Note: The migrations will be applied as well.

```sh
docker-compose up
```

### Forked Test Network

In order to run the `e2e forked_network` tests you have to have [anvil](https://github.com/foundry-rs/foundry) installed,
if you haven't installed `anvil` yet, refer to `foundry`'s [installation guide](https://book.getfoundry.sh/getting-started/installation) to get started.

All `forked_node` tests will require a `FORK_MAINNET_URL`, you can refer to [Chainlist](https://chainlist.org/chain/1) to find some publicly available RPCs (terms and conditions may apply).
A subset of the `forked_node` tests will require a `FORK_GNOSIS_URL`, refer to the list of [Gnosis RPC Providers](https://docs.gnosischain.com/tools/RPC%20Providers/) for publicly available nodes.

## Profiling

All binaries are compiled with support for [tokio-console](https://github.com/tokio-rs/console) by default to allow you to look inside the tokio runtime.
However, this feature is not enabled at runtime by default because it comes with a pretty significant memory overhead. To enable it you just have to set the environment variable `TOKIO_CONSOLE=true` and run the binary you want to instrument.

You can install and run `tokio-console` with:
```bash
cargo install --locked tokio-console
tokio-console
```

## Heap Profiling

All binaries use jemalloc as the default memory allocator with built-in heap profiling support. Profiling is enabled at runtime via the `MALLOC_CONF` environment variable, allowing you to analyze memory usage in production environments without recompiling or restarting services.

**Note:** You can optionally use mimalloc instead of jemalloc by building with `--features mimalloc-allocator`, but this disables heap profiling capability.

### Enabling Heap Profiling

To enable heap profiling, run services with the `MALLOC_CONF` environment variable set:
```bash
MALLOC_CONF="prof:true,prof_active:true,lg_prof_sample:22"
```

When profiling is enabled, each binary opens a UNIX socket at `/tmp/heap_dump_<binary_name>.sock`.

### Generating Heap Dumps

Connect to the socket and send the "dump" command:

```bash
# From Kubernetes
kubectl exec <pod> -n <namespace> -- sh -c "echo dump | nc -U /tmp/heap_dump_orderbook.sock" > heap.pprof

# From Docker
docker exec <container> sh -c "echo dump | nc -U /tmp/heap_dump_orderbook.sock" > heap.pprof
```

### Analyzing Heap Dumps

The dumps are in pprof format and can be analyzed using Google's pprof tool:

```bash
# Install pprof
go install github.com/google/pprof@latest

# Interactive web UI
pprof -http=:8080 heap.pprof

# Command-line analysis
pprof -top heap.pprof
```

## Changing Log Filters

It's possible to change the tracing log filter while the process is running. This can be useful to debug an error that requires more verbose logs but which might no longer appear after restarting the system.

Each process opens a UNIX socket at `/tmp/log_filter_override_<program_name>_<pid>.sock`. To change the log filter connect to it with `nc -U <path>` and enter a new log filter.
You can also reset the log filter to the filter the program was initially started with by entering `reset`.

See [here](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives) for documentation on the supported log filter format.
