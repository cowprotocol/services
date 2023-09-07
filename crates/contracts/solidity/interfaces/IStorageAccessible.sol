// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title Storage accessible interface.
interface IStorageAccessible {
    function simulateDelegatecall(address reader, bytes memory call) external returns (bytes memory result);
}
