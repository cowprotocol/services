// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @dev Magic value returned for valid signatures
bytes4 constant ERC1271_MAGICVALUE = 0x1626ba7e;

/// @title ERC-1271 signature validation interface
interface IERC1271 {
    function isValidSignature(bytes32 hash, bytes memory signature) external view returns (bytes4 magicValue);
}
