# contracts

The library part of this crate contains `ethcontract` generated bindings to the smart contracts we use.

## `vendor` script

A `[[bin]]` script for vendoring smart contract json artifacts that ethcontract uses to generate its bindings. We keep these in the repository to make the build more deterministic and robust.

## Steps to add a new contract

- In `vendor.rs` extend `ARTIFACTS` with the package and contract name.
- Run the vendor binary for example with `(cd contracts; cargo run --bin vendor --features bin)`. This creates a new json file for this contract in the `artifacts` folder.
- In `build.rs` add a `generate_contract` call for the contract. This creates the ethcontract generated rust code file.
- In `lib.rs` add an `include!` call for the contract. This imports the rust code into the library.
