// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "forge-std/Script.sol";
import {Balances} from "../src/Balances.sol";

contract DeployBalances is Script {
    function run() public {
        vm.startBroadcast();

        // Deploy Balances contract with CREATE2 for deterministic address
        Balances balances = new Balances{salt: bytes32(0)}();
        
        console.log("Balances deployed at:", address(balances));

        vm.stopBroadcast();
    }
}
