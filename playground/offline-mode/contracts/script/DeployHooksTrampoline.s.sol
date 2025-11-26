// SPDX-License-Identifier: MIT
pragma solidity ^0.8.17;

import "forge-std/Script.sol";
import {HooksTrampoline} from "@cowprotocol/hooks-trampoline/HooksTrampoline.sol";

contract DeployHooksTrampoline is Script {
    // Deterministic salt for CREATE2
    bytes32 constant HOOKS_TRAMPOLINE_SALT = keccak256("cowswap-hooks-trampoline");

    function run() public {
        // Read settlement address from environment
        address settlement = vm.envAddress("SETTLEMENT");

        vm.startBroadcast();

        // Deploy HooksTrampoline contract with CREATE2 for deterministic address
        HooksTrampoline hooksTrampoline = new HooksTrampoline{salt: HOOKS_TRAMPOLINE_SALT}(settlement);

        console.log("HooksTrampoline deployed at:", address(hooksTrampoline));

        vm.stopBroadcast();
    }
}
