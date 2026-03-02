// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

IVault constant VAULT = IVault(0xBA12222222228d8Ba445958a75a0704d566BF2C8);

/// @title CoW protocol settlement contract interface
interface IVault {
    function hasApprovedRelayer(address user, address relayer) external view returns (bool);
    function getInternalBalance(address user, address[] calldata tokens) external view returns (uint256[] memory);
}
