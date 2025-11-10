// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "forge-std/Script.sol";

// Minimal Signatures interface for deployment
contract Signatures {
    constructor() {}
}

contract DeploySignatures is Script {
    function run() public {
        vm.startBroadcast();

        // Deploy Signatures contract with CREATE2 for deterministic address
        Signatures signatures = new Signatures{salt: bytes32(0)}();
        
        console.log("Signatures deployed at:", address(signatures));

        vm.stopBroadcast();
    }
}
