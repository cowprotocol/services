// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "forge-std/Script.sol";

// Minimal Signatures interface for deployment
contract Signatures {
    constructor() {}
}

contract DeploySignatures is Script {
    // Deterministic salt for CREATE2
    bytes32 constant SIGNATURES_SALT = keccak256("cowswap-signatures");

    function run() public {
        vm.startBroadcast();

        // Deploy Signatures contract with CREATE2 for deterministic address
        Signatures signatures = new Signatures{salt: SIGNATURES_SALT}();

        console.log("Signatures deployed at:", address(signatures));

        vm.stopBroadcast();
    }
}
