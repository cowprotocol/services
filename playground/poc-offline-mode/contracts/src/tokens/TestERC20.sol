// SPDX-License-Identifier: MIT
pragma solidity ^0.8.26;

import {ERC20} from "solmate/tokens/ERC20.sol";

/// @title TestERC20
/// @notice Simple ERC20 token for testing (USDC, DAI, etc.)
contract TestERC20 is ERC20 {
    constructor(
        string memory name,
        string memory symbol,
        uint8 decimals,
        uint256 initialSupply
    ) ERC20(name, symbol, decimals) {
        _mint(msg.sender, initialSupply);
    }

    /// @notice Mint tokens to an address (for testing)
    function mint(address to, uint256 amount) external {
        _mint(to, amount);
    }

    /// @notice Burn tokens from an address (for testing)
    function burn(address from, uint256 amount) external {
        _burn(from, amount);
    }
}
