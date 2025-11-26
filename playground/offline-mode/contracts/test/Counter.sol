// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

/**
 * @title Counter
 * @dev Simple test contract for HooksTrampoline testing
 * This contract demonstrates a basic hook that increments a counter when called
 */
contract Counter {
    uint256 public counter;
    address public lastCaller;

    event CounterIncremented(address indexed caller, uint256 newValue);

    /**
     * @dev Increment the counter
     * This function is designed to be called via HooksTrampoline
     */
    function increment() external {
        counter++;
        lastCaller = msg.sender;
        emit CounterIncremented(msg.sender, counter);
    }

    /**
     * @dev Get the current counter value
     */
    function getCounter() external view returns (uint256) {
        return counter;
    }

    /**
     * @dev Reset the counter (for testing purposes)
     */
    function reset() external {
        counter = 0;
        lastCaller = address(0);
    }
}
