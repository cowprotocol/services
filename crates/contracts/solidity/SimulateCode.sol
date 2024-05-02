// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import { IStorageAccessible } from "./interfaces/IStorageAccessible.sol";

/// @title A contract for simulating arbitrary code in the context of a
/// `StorageAccessible` contract.
contract SimulateCode {
    /// @dev This looks like a constructor but it is not... In fact, nodes
    /// support `eth_call`s for contract creation and **return the code of the
    /// contract that would be created**. This means we can use constructors to
    /// execute arbitrary code on the current state of the EVM, and "manually"
    /// return with some inline assembly that data (as this is the mechanism
    /// used for contract creation).
    ///
    /// The contract does this to:
    /// 1. Deploy some arbitrary contract code
    /// 2. Use the `StorageAccessible` pattern to execute the contract code
    ///    deployed in step 1. within the another contract context (usually the
    ///    settlement contract - which implements this pattern)
    ///
    /// This allows us to make use of `StorageAccessible` without actually
    /// deploying a contract :).
    ///
    /// Returns the return data from the simulation code.
    ///
    /// @param target - The `StorageAccessible` implementation.
    /// @param code - Creation code for the reader contract.
    /// @param call - The calldata to pass in the `DELEGATECALL` simulation.
    constructor(
        IStorageAccessible target,
        bytes memory code,
        bytes memory call
    ) {
        address implementation;
        assembly {
            implementation := create(callvalue(), add(code, 32), mload(code))
        }

        bytes memory result = target.simulateDelegatecall(implementation, call);
        assembly {
            return(add(32, result), mload(result))
        }
    }
}
