// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title Minimal EIP-4626 wrapper for testing recursive vault pricing.
/// @dev Wraps another ERC-4626 vault (or any ERC-20) with a fixed 1:1
///      conversion rate.  Not a real vault – just enough to satisfy
///      `asset()`, `decimals()`, `convertToAssets()`, `balanceOf()`,
///      `approve()`, and `transfer()` so the e2e pricing pipeline works.
contract MockERC4626Wrapper {
    address public immutable asset;
    uint8   public immutable decimals;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    constructor(address _asset, uint8 _decimals) {
        asset    = _asset;
        decimals = _decimals;
    }

    // ── EIP-4626 view ──────────────────────────────────────────────────

    function convertToAssets(uint256 shares) external pure returns (uint256) {
        return shares; // 1:1 conversion
    }

    // ── Minimal ERC-20 surface needed by the protocol ──────────────────

    function transfer(address to, uint256 amount) external returns (bool) {
        balanceOf[msg.sender] -= amount;
        balanceOf[to]         += amount;
        return true;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        allowance[from][msg.sender] -= amount;
        balanceOf[from]             -= amount;
        balanceOf[to]               += amount;
        return true;
    }

    // ── Test helper ────────────────────────────────────────────────────

    function mint(address to, uint256 amount) external {
        balanceOf[to] += amount;
    }
}
