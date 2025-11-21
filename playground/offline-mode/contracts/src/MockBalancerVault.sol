// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

/// @notice Minimal mock of Balancer Vault for offline CoW Protocol testing
contract MockBalancerVault {
    function hasApprovedRelayer(address, address) external pure returns (bool) {
        return false;
    }

    function getInternalBalance(address, address[] calldata)
        external
        pure
        returns (uint256[] memory)
    {
        return new uint256[](0);
    }
}
