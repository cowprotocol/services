// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

IVaultRelayer constant VAULT_RELAYER = IVaultRelayer(0xC92E8bdf79f0507f65a392b0ab4667716BFE0110);

struct Transfer {
    address account;
    address token;
    uint256 amount;
    bytes32 balance;
}

/// @title CoW protocol settlement contract interface
interface IVaultRelayer {
    function transferFromAccounts(Transfer[] calldata transfers) external;
}
