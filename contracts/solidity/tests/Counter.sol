// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// Minimal ERC20 interface for balance checks.
interface IERC20 {
    function balanceOf(address owner) external view returns (uint256);
}

/// @title Helper contract to count how many times a function is called
contract Counter {
    mapping(string => uint256) public counters;

    function incrementCounter(string memory key) public {
        counters[key] += 1;
    }

    function setCounterToBalance(
        string memory key,
        address token,
        address owner
    ) public {
        counters[key] = IERC20(token).balanceOf(owner);
    }
}
