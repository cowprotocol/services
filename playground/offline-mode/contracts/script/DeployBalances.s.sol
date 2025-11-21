// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "forge-std/Script.sol";
import {Balances} from "../src/Balances.sol";

contract DeployBalances is Script {
    // Deterministic salt for CREATE2
    bytes32 constant BALANCES_SALT = keccak256("cowswap-balances");

    function run() public {
        vm.startBroadcast();

        // Deploy Balances contract with CREATE2 for deterministic address
        Balances balances = new Balances{salt: BALANCES_SALT}();

        console.log("Balances deployed at:", address(balances));

        vm.stopBroadcast();
    }
}
