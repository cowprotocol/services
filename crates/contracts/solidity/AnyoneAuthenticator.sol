// SPDX-License-Identifier: MIT
pragma solidity ^0.8.16;

contract AnyoneAuthenticator {
    function isSolver(address) external pure returns (bool) {
        return true;
    }
}
