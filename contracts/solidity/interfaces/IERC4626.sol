// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title ERC-4626 vaults interface
interface IERC4626 {
    function asset() external view returns (address assetTokenAddress);
    function convertToAssets(uint256 shares) external view returns (uint256 assets);
}
