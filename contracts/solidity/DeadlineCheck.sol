// SPDX-License-Identifier: UNLICENSED
pragma solidity ^0.8.20;

contract DeadlineCheck {
    error DeadlineExceeded(uint256 currentBlock, uint256 deadline);

    function checkDeadline(uint256 deadline) external view {
        if (block.number > deadline) {
            revert DeadlineExceeded(block.number, deadline);
        }
    }
}
