# contracts

The library part of this crate contains `ethcontract` generated bindings to the smart contracts we use.

## `deploy` Script

A `[[bin]]` script for deploying Gnosis Protocol v2 contracts to a test network.
This script requires a test node such as Ganache (with at least Istanbul Hardfork) listening on `127.0.0.1:8545`.
It can be run from the repository root:

```
$ (cd contracts; cargo run --bin deploy --features bin)
```

This will generate `$CONTRACT_NAME.addr` files in the `target/deploy` directory.
The `build.rs` script uses these files to inject test network deployed addresses
into the generated bindings so `Contract::deployed()` methods work as expected
for E2E tests when connected to a local network.

Note that the `contracts` crate needs to be re-built after running the `deploy`
script to generate bindings with the injected test network addresses. This is
done automatically on `cargo build` by leveraging the `cargo:rerun-if-changed`
build script feature.

## `vendor` script

A `[[bin]]` script for vendoring smart contract json artifacts that ethcontract uses to generate its bindings. We keep these in the repository to make the build more deterministic and robust.

## Steps to add a new contract

- In `vendor.rs` extend `ARTIFACTS` with the package and contract name.
- Run the vendor binary for example with `(cd contracts; cargo run --bin vendor --features bin)`. This creates a new json file for this contract in the `artifacts` folder.
- In `build.rs` add a `generate_contract` call for the contract. This creates the ethcontract generated rust code file.
- In `lib.rs` add an `include!` call for the contract. This imports the rust code into the library.
