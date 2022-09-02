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
