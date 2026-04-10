// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import "solidity/Signatures.sol";
import "forge-std/Script.sol";

contract DeploySignatures is Script {
    function run() public {
        vm.startBroadcast();

        // Use deterministic salt to deploy the contract.
        new Signatures{salt: ""}();

        vm.stopBroadcast();
    }
}
