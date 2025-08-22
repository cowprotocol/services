// SPDX-License-Identifier: MIT
pragma solidity ^0.8.30;

import "solidity/Balances.sol";
import "forge-std/Script.sol";

contract DeployBalances is Script {
    function run() public {
        vm.startBroadcast();

        // Use deterministic salt to deploy the contract.
        new Balances{salt: ""}();

        vm.stopBroadcast();
    }
}
