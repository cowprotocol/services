// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "forge-std/Script.sol";
import {DeploymentUtils} from "./Utils.sol";

// Import Balances from the main codebase using @bleu-services remapping
// This contract has the balance() function required by the orderbook
import {Balances} from "@bleu-services/Balances.sol";

contract DeployTradeSimulator is Script {
    using DeploymentUtils for bytes;

    // Deterministic salt for CREATE2
    bytes32 constant BALANCES_SALT = keccak256("cowswap-balances-contract");

    function run() public {
        vm.startBroadcast();

        // Deploy Balances contract using CREATE2
        // This contract has the balance() function required by the orderbook
        bytes memory bytecode = type(Balances).creationCode;
        address balances = DeploymentUtils.deployWithCreate2(bytecode, BALANCES_SALT);

        require(balances != address(0), "Balances deployment failed");

        console.log("Balances contract deployed at:", balances);

        vm.stopBroadcast();
    }
}
