// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title MockERC4626Wrapper
/// @notice Minimal EIP-4626 wrapper for testing recursive vault pricing.
/// @dev Wraps another ERC-4626 vault (or any ERC-20) with a configurable
/// conversion rate. Not a real vault -- just enough to satisfy `asset()`,
/// `decimals()`, `convertToAssets()`, `balanceOf()`, `approve()`, and
/// `transfer()` so the e2e pricing pipeline works.
contract MockERC4626Wrapper {
    address public immutable asset;
    uint8 public immutable decimals;
    uint256 public immutable rateNumerator;
    uint256 public immutable rateDenominator;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    constructor(address _asset, uint8 _decimals, uint256 _rateNumerator, uint256 _rateDenominator) {
        asset = _asset;
        decimals = _decimals;
        rateNumerator = _rateNumerator;
        rateDenominator = _rateDenominator;
    }

    /// @notice Returns the equivalent amount of underlying assets for the
    /// given number of vault shares, scaled by the configured rate.
    function convertToAssets(uint256 shares) external view returns (uint256) {
        return shares * rateNumerator / rateDenominator;
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        allowance[from][msg.sender] -= amount;
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        return true;
    }

    /// @notice Mints tokens to an address. Only for testing.
    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }
}
