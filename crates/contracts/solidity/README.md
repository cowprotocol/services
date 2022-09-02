# Support Solidity Code

This directory contains Solidity for support contracts used by the services.
These contracts provide bytecode that is used by the services, either with `eth_call` with state overrides, or deployed, and subsequently called into, with `trace_callMany`.

## Building

The compiled ABI and bytecodes are compiled and commited into the repository.
This makes it easier to build the services repository without additional steps, and the bytecode here is not expected to change often.

That being said, if changes to the contracts were made, then you need to rebuild them and commit the updated artifacts to the repository.

### Requirements

- Docker
- `jq`

### Command

```sh
make
```

## Adding New Contracts

The `Makefile` was designed to be somewhat general.
You should be able to just append a new Solidity file to the `CONTRACTS` variable definition:

```diff
--- a/crates/contracts/solidity/Makefile
+++ b/crates/contracts/solidity/Makefile
@@ -7,7 +7,7 @@ SOLFLAGS := --overwrite --abi --bin --bin-runtime --metadata-hash none --optimiz
 TARGETDIR   := ../../../target/solidity
 ARTIFACTDIR := ../artifacts
 
-CONTRACTS := Existing.sol Contracts.sol
+CONTRACTS := Existing.sol Contracts.sol NewContract.sol
 ARTIFACTS := $(patsubst %.sol,$(ARTIFACTDIR)/%.json,$(CONTRACTS))
 
 .PHONY: artifacts
```
