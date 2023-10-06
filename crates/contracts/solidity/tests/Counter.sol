// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/// @title Helper contract to count how many times a function is called
contract Counter {
    mapping(string => uint256) public counters;

    function incrementCounter(string memory key) public {
        counters[key] += 1;
    }
}
