// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

contract Counter {
    mapping(string => uint256) public counters;

    function incrementCounter(string memory key) public {
        counters[key] += 1;
    }
}
