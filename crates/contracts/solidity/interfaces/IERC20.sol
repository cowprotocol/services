// SPDX-License-Identifier: MIT
pragma solidity ^0.8.16;

/// @title ERC-20 token interface
interface IERC20 {
    function balanceOf(address) external view returns (uint256);
    function allowance(address owner, address spender) external view returns (uint256);

    function approve(address, uint256) external returns (bool);
    function transfer(address, uint256) external returns (bool);
    function transferFrom(address, address, uint256) external returns (bool);
}

/// @title Mintable token interface
interface IMintableERC20 is IERC20 {
    function mint(address, uint256) external returns (bool);
}

/// @title Native token interface
interface INativeERC20 is IERC20 {
    function withdraw(uint256) external;
    function deposit() external payable;
}
